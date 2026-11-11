use super::SowEngine;
use crate::game::GamePhase;
use crate::player::PlayerId;
use crate::protocol::{MARKET_TROOPS_PER_LOT, MarketSide};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOrder {
    pub owner: PlayerId,
    pub side: MarketSide,
    pub lots: u32,
    pub gold_per_lot: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketFailure {
    Unavailable,
    InvalidOrder,
    InsufficientStock,
    InsufficientGold,
    InsufficientTroops,
    TroopCapacity,
    PriceChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarketOutcome {
    Traded {
        side: MarketSide,
        lots: u32,
        gold: u32,
    },
    Posted,
    Cancelled,
    Rejected {
        reason: MarketFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MarketFeedback {
    pub request_id: u32,
    pub outcome: MarketOutcome,
}

#[derive(Serialize)]
pub struct MarketView {
    pub side: MarketSide,
    pub lots: u32,
    pub troops_per_lot: u32,
    pub available_lots: u64,
    pub best_price: Option<u32>,
    pub quote_gold: Option<u32>,
    pub quote_failure: Option<MarketFailure>,
    pub order: Option<MarketOrder>,
    pub feedback: Option<MarketFeedback>,
}

type MarketResult = Result<MarketOutcome, MarketFailure>;

struct MarketFill {
    order_index: usize,
    lots: u32,
    gold: u32,
}

fn whole_lots(amount: f64, per_lot: u32) -> u32 {
    if amount.is_finite() && per_lot != 0 {
        ((amount as u64) / u64::from(per_lot)).min(u64::from(u32::MAX)) as u32
    } else {
        0
    }
}

impl SowEngine {
    pub fn market_enabled_for(&self, player: PlayerId) -> bool {
        let config = &self.state.config;
        self.state.phase == GamePhase::Playing
            && !config.ranked_1v1
            && !config.tutorial
            && self.campaign_faction_ids.is_empty()
            && matches!(
                config.game_mode.as_str(),
                "FFA" | "Teams" | "HumansVsNations"
            )
            && self
                .state
                .player(player)
                .is_some_and(|p| p.is_human() && !p.is_ai_controlled && p.alive && p.has_spawned)
    }

    fn market_partners(&self, a: PlayerId, b: PlayerId) -> bool {
        if !self.market_enabled_for(a)
            || !self.market_enabled_for(b)
            || !self.resources_can_move(a, b)
            || !self.resources_can_move(b, a)
        {
            return false;
        }
        let (a, b) = (self.state.player(a).unwrap(), self.state.player(b).unwrap());
        if self.state.config.game_mode == "FFA" {
            a.alliances.contains(&b.id) && b.alliances.contains(&a.id)
        } else {
            a.team.is_some() && a.team == b.team
        }
    }

    fn market_backing(&self, order: MarketOrder) -> u32 {
        if !self.market_enabled_for(order.owner) {
            return 0;
        }
        let player = self.state.player(order.owner).unwrap();
        let backed = match order.side {
            MarketSide::Buy => whole_lots(player.gold, order.gold_per_lot),
            MarketSide::Sell => whole_lots(self.troops_spare(order.owner), MARKET_TROOPS_PER_LOT),
        };
        order.lots.min(backed)
    }

    pub(super) fn prune_market_orders(&mut self) {
        let mut orders = std::mem::take(&mut self.market_orders);
        orders.retain_mut(|order| {
            order.lots = self.market_backing(*order);
            order.lots > 0
        });
        self.market_orders = orders;
    }

    fn market_indices(&self, side: MarketSide) -> Vec<usize> {
        let mut indices: Vec<_> = self
            .market_orders
            .iter()
            .enumerate()
            .filter_map(|(i, order)| (order.side == side).then_some(i))
            .collect();
        // Vector order is publication order, including intents in the same turn.
        indices.sort_unstable_by_key(|&i| {
            let price = self.market_orders[i].gold_per_lot;
            (
                if side == MarketSide::Buy {
                    u32::MAX - price
                } else {
                    price
                },
                i,
            )
        });
        indices
    }

    fn market_stock(&self, player: PlayerId, side: MarketSide) -> Vec<(usize, u32)> {
        let opposite = match side {
            MarketSide::Buy => MarketSide::Sell,
            MarketSide::Sell => MarketSide::Buy,
        };
        self.market_indices(opposite)
            .into_iter()
            .filter_map(|index| {
                let order = self.market_orders[index];
                if !self.market_partners(player, order.owner) {
                    return None;
                }
                let mut lots = self.market_backing(order);
                if order.side == MarketSide::Buy {
                    lots = lots.min(whole_lots(
                        self.troop_room(order.owner),
                        MARKET_TROOPS_PER_LOT,
                    ));
                }
                (lots > 0).then_some((index, lots))
            })
            .collect()
    }

    fn plan_market_trade(
        &self,
        player: PlayerId,
        side: MarketSide,
        lots: u32,
        stock: &[(usize, u32)],
    ) -> Result<(u32, Vec<MarketFill>), MarketFailure> {
        use MarketFailure::*;
        if !self.market_enabled_for(player) {
            return Err(Unavailable);
        }
        if lots == 0 {
            return Err(InvalidOrder);
        }
        let (capacity, reason) = match side {
            MarketSide::Buy => (self.troop_room(player), TroopCapacity),
            MarketSide::Sell => (self.troops_spare(player), InsufficientTroops),
        };
        if whole_lots(capacity, MARKET_TROOPS_PER_LOT) < lots {
            return Err(reason);
        }

        let mut remaining = lots;
        let mut fills = Vec::new();
        let mut total = 0u32;
        for &(index, available) in stock {
            let take = remaining.min(available);
            let gold = take
                .checked_mul(self.market_orders[index].gold_per_lot)
                .ok_or(InvalidOrder)?;
            total = total.checked_add(gold).ok_or(InvalidOrder)?;
            fills.push(MarketFill {
                order_index: index,
                lots: take,
                gold,
            });
            remaining -= take;
            if remaining == 0 {
                break;
            }
        }
        if remaining != 0 {
            return Err(InsufficientStock);
        }
        if side == MarketSide::Buy
            && self.gold_transferable(player, f64::from(total)) < f64::from(total)
        {
            return Err(InsufficientGold);
        }
        Ok((total, fills))
    }

    pub fn market_view(&self, player: PlayerId, side: MarketSide, lots: u32) -> MarketView {
        let stock = self.market_stock(player, side);
        let quote = self.plan_market_trade(player, side, lots, &stock);
        MarketView {
            side,
            lots,
            troops_per_lot: MARKET_TROOPS_PER_LOT,
            available_lots: stock.iter().map(|&(_, lots)| u64::from(lots)).sum(),
            best_price: stock
                .first()
                .map(|&(index, _)| self.market_orders[index].gold_per_lot),
            quote_failure: quote.as_ref().err().copied(),
            quote_gold: quote.ok().map(|(gold, _)| gold),
            order: self
                .market_orders
                .iter()
                .find(|order| order.owner == player)
                .copied(),
            feedback: self.market_feedback.get(&player).copied(),
        }
    }

    pub(crate) fn record_market_feedback(
        &mut self,
        player: PlayerId,
        request_id: u32,
        result: MarketResult,
    ) {
        if self
            .state
            .player(player)
            .is_some_and(|p| p.is_human() && !p.is_ai_controlled)
        {
            let outcome = result.unwrap_or_else(|reason| MarketOutcome::Rejected { reason });
            self.market_feedback.insert(
                player,
                MarketFeedback {
                    request_id,
                    outcome,
                },
            );
        }
    }

    pub(crate) fn place_market_order(
        &mut self,
        owner: PlayerId,
        side: MarketSide,
        lots: u32,
        gold_per_lot: u32,
    ) -> MarketResult {
        use MarketFailure::*;
        if !self.market_enabled_for(owner) {
            return Err(Unavailable);
        }
        if lots == 0 || gold_per_lot == 0 || lots.checked_mul(gold_per_lot).is_none() {
            return Err(InvalidOrder);
        }
        let order = MarketOrder {
            owner,
            side,
            lots,
            gold_per_lot,
        };
        if self.market_backing(order) < lots {
            return Err(if side == MarketSide::Buy {
                InsufficientGold
            } else {
                InsufficientTroops
            });
        }
        self.market_orders
            .retain(|previous| previous.owner != owner);
        self.market_orders.push(order);
        Ok(MarketOutcome::Posted)
    }

    pub(crate) fn cancel_market_order(&mut self, owner: PlayerId) -> MarketResult {
        if !self.market_enabled_for(owner) {
            return Err(MarketFailure::Unavailable);
        }
        self.market_orders.retain(|order| order.owner != owner);
        Ok(MarketOutcome::Cancelled)
    }

    pub(crate) fn trade_at_market(
        &mut self,
        player: PlayerId,
        side: MarketSide,
        lots: u32,
        limit_gold: u32,
    ) -> MarketResult {
        self.prune_market_orders();
        let stock = self.market_stock(player, side);
        let (gold, fills) = self.plan_market_trade(player, side, lots, &stock)?;
        if (side == MarketSide::Buy && gold > limit_gold)
            || (side == MarketSide::Sell && gold < limit_gold)
        {
            return Err(MarketFailure::PriceChanged);
        }
        for fill in fills {
            let order = &mut self.market_orders[fill.order_index];
            order.lots -= fill.lots;
            let (buyer, seller) = match side {
                MarketSide::Buy => (player, order.owner),
                MarketSide::Sell => (order.owner, player),
            };
            self.settle_market_fill(buyer, seller, fill.lots, fill.gold);
        }
        self.prune_market_orders();
        Ok(MarketOutcome::Traded { side, lots, gold })
    }

    fn settle_market_fill(&mut self, buyer: PlayerId, seller: PlayerId, lots: u32, gold: u32) {
        self.move_resources(buyer, seller, f64::from(gold), 0.0);
        self.move_resources(
            seller,
            buyer,
            0.0,
            f64::from(lots) * f64::from(MARKET_TROOPS_PER_LOT),
        );
    }

    pub(super) fn clear_market_orders(&mut self) {
        self.prune_market_orders();
        if self.market_orders.len() < 2 {
            return;
        }
        let buys = self.market_indices(MarketSide::Buy);
        let sells = self.market_indices(MarketSide::Sell);
        // ponytail: scan eligible sellers per buyer. Index by partner if human order counts warrant it.
        for buy in buys {
            for &sell in &sells {
                let (bid, ask) = (self.market_orders[buy], self.market_orders[sell]);
                if bid.lots == 0 || bid.gold_per_lot < ask.gold_per_lot {
                    break;
                }
                if ask.lots == 0 || !self.market_partners(bid.owner, ask.owner) {
                    continue;
                }
                let room = whole_lots(
                    self.troops_transferable(ask.owner, bid.owner, f64::MAX),
                    MARKET_TROOPS_PER_LOT,
                );
                let lots = self.market_backing(bid).min(ask.lots).min(room);
                if lots == 0 {
                    continue;
                }
                let price = self.market_orders[buy.min(sell)].gold_per_lot;
                self.settle_market_fill(bid.owner, ask.owner, lots, lots * price);
                self.market_orders[buy].lots -= lots;
                self.market_orders[sell].lots -= lots;
            }
        }
        self.prune_market_orders();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::player::Player;
    use crate::protocol::{GameplayIntent, StampedIntent, Team, Turn};
    use crate::water_components::WaterComponents;

    fn engine() -> SowEngine {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 8, 8, config.clone());
        state.phase = GamePhase::Playing;
        for id in 1..=4 {
            let mut player = Player::new_human(id, format!("Player {id}"), [1.0; 3], &config);
            player.has_spawned = true;
            player.gold = 100.0;
            player.troops = 501.0;
            player.max_troops = 1_000.0;
            player.alliances = (1..=4).filter(|other| *other != id).collect();
            state.register_player(player);
            state.set_tile_owner(u32::from(id), 1, id);
        }
        SowEngine::new(state, WaterComponents::default())
    }

    fn balances(engine: &SowEngine) -> Vec<(f64, f64)> {
        engine
            .state
            .players
            .iter()
            .map(|p| (p.gold, p.troops))
            .collect()
    }

    #[test]
    fn market_quote_splits_fills_and_rejects_a_stale_total_atomically() {
        let mut e = engine();
        e.place_market_order(2, MarketSide::Sell, 1, 18).unwrap();
        e.place_market_order(3, MarketSide::Sell, 3, 20).unwrap();
        assert_eq!(e.market_view(1, MarketSide::Buy, 2).quote_gold, Some(38));
        e.trade_at_market(4, MarketSide::Buy, 1, 18).unwrap();
        let (before, book) = (balances(&e), e.market_orders.clone());
        assert_eq!(
            e.trade_at_market(1, MarketSide::Buy, 2, 38),
            Err(MarketFailure::PriceChanged)
        );
        assert_eq!((balances(&e), e.market_orders.clone()), (before, book));
        e.place_market_order(2, MarketSide::Sell, 1, 19).unwrap();
        assert_eq!(e.market_view(1, MarketSide::Buy, 2).quote_gold, Some(39));
        e.trade_at_market(1, MarketSide::Buy, 2, 39).unwrap();
        assert_eq!(
            balances(&e),
            vec![(61.0, 701.0), (137.0, 301.0), (120.0, 401.0), (82.0, 601.0)]
        );
    }

    #[test]
    fn standing_orders_use_the_older_price_and_trim_live_backing() {
        let mut e = engine();
        e.place_market_order(1, MarketSide::Buy, 4, 20).unwrap();
        e.place_market_order(2, MarketSide::Sell, 4, 12).unwrap();
        e.state.player_mut(1).unwrap().troops = 801.0;
        e.clear_market_orders();
        assert_eq!(balances(&e)[0], (80.0, 901.0));
        assert_eq!(balances(&e)[1], (120.0, 401.0));
        assert_eq!(
            e.market_orders.iter().map(|o| o.lots).collect::<Vec<_>>(),
            [3, 3]
        );
        e.apply_intents(&[
            StampedIntent {
                player_id: 1,
                intent: GameplayIntent::SendResources {
                    target_player: 4,
                    gold: 61.0,
                    troops: 0.0,
                },
            },
            StampedIntent {
                player_id: 2,
                intent: GameplayIntent::SendResources {
                    target_player: 4,
                    gold: 0.0,
                    troops: 200.0,
                },
            },
        ]);
        e.clear_market_orders();
        assert_eq!(e.market_orders.len(), 1);
        assert_eq!(e.market_orders[0].lots, 2);
        e.state.player_mut(2).unwrap().troops = 501.0;
        e.clear_market_orders();
        assert_eq!(e.market_orders[0].lots, 2);
        assert_eq!(
            e.place_market_order(2, MarketSide::Sell, 2, 0),
            Err(MarketFailure::InvalidOrder)
        );
        assert_eq!(e.market_orders[0].gold_per_lot, 12);
        e.place_market_order(2, MarketSide::Sell, 2, 13).unwrap();
        assert_eq!(e.market_orders.len(), 1);
        assert_eq!(e.market_orders[0].gold_per_lot, 13);
        assert_eq!(
            e.trade_at_market(3, MarketSide::Sell, 5, 1),
            Err(MarketFailure::InsufficientStock)
        );
        e.place_market_order(3, MarketSide::Buy, 1, 10).unwrap();
        e.trade_at_market(2, MarketSide::Sell, 1, 10).unwrap();
        assert_eq!(balances(&e)[1], (130.0, 401.0));
        e.cancel_market_order(2).unwrap();
        assert!(e.market_orders.is_empty());
    }

    #[test]
    fn market_filters_direct_allies_and_uses_teams_in_team_modes() {
        let mut e = engine();
        e.place_market_order(2, MarketSide::Sell, 2, 12).unwrap();
        e.state
            .player_mut(1)
            .unwrap()
            .alliances
            .retain(|&ally| ally != 2);
        assert_eq!(e.market_view(1, MarketSide::Buy, 1).available_lots, 0);
        assert_eq!(e.market_view(3, MarketSide::Buy, 1).available_lots, 2);
        for mode in ["Teams", "HumansVsNations"] {
            e.state.config.game_mode = mode.into();
            e.state.player_mut(1).unwrap().team = Some(Team::Red);
            e.state.player_mut(2).unwrap().team = Some(Team::Red);
            e.state.player_mut(3).unwrap().team = Some(Team::Blue);
            assert_eq!(e.market_view(1, MarketSide::Buy, 1).available_lots, 2);
            assert_eq!(e.market_view(3, MarketSide::Buy, 1).available_lots, 0);
        }
        e.state.player_mut(2).unwrap().is_ai_controlled = true;
        assert_eq!(e.market_view(1, MarketSide::Buy, 1).available_lots, 0);
        e.state.config.ranked_1v1 = true;
        assert_eq!(
            e.place_market_order(1, MarketSide::Buy, 1, 10),
            Err(MarketFailure::Unavailable)
        );
    }

    #[test]
    fn market_turns_replay_with_fifo_and_restore_pending_orders_from_checkpoint() {
        let place = |player_id, side, lots| StampedIntent {
            player_id,
            intent: GameplayIntent::PlaceMarketOrder {
                request_id: 1,
                side,
                lots,
                gold_per_lot: 10,
            },
        };
        let turns = [
            Turn {
                turn_number: 0,
                intents: vec![place(3, MarketSide::Sell, 2), place(2, MarketSide::Sell, 2)],
            },
            Turn {
                turn_number: 1,
                intents: vec![place(1, MarketSide::Buy, 3)],
            },
        ];
        let recording: Vec<Turn> =
            bincode::deserialize(&bincode::serialize(&turns[..]).unwrap()).unwrap();
        let mut original = engine();
        let mut replay = engine();
        original.apply_intents(&turns[0].intents);
        original.tick();
        replay.apply_intents(&recording[0].intents);
        replay.tick();
        let checkpoint = bincode::serialize(&replay).unwrap();
        replay = bincode::deserialize(&checkpoint).unwrap();
        original.apply_intents(&turns[1].intents);
        original.tick();
        replay.apply_intents(&recording[1].intents);
        replay.tick();
        assert_eq!(balances(&original), balances(&replay));
        assert_eq!(original.market_orders, replay.market_orders);
        assert_eq!(original.market_orders.len(), 1);
        assert_eq!(
            (
                original.market_orders[0].owner,
                original.market_orders[0].lots
            ),
            (2, 1)
        );
    }
}
