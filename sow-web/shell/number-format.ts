// Compact totals shared by the account header and match results.
// Account balances are u64: through 18.4E, the label needs at most six glyphs (999.9K).
export function formatCompactNumber(value: number | string | null | undefined): string {
    const amount = Number(value);
    if (!Number.isFinite(amount) || amount <= 0) return "0";
    if (amount < 1000) return String(Math.round(amount));
    const suffixes = ["", "K", "M", "B", "T", "Q", "E"];
    let unit = Math.min(suffixes.length - 1, Math.floor(Math.log10(amount) / 3));
    let scaled = Math.round((amount / 1000 ** unit) * 10) / 10;
    if (scaled >= 1000 && unit < suffixes.length - 1) {
        unit += 1;
        scaled = Math.round((amount / 1000 ** unit) * 10) / 10;
    }
    return String(scaled) + suffixes[unit];
}
