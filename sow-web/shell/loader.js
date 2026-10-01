/**
 * Web boot loader — splash + progress bar until Rust publishes loader_done.
 */
(function () {
    'use strict';

    const FADEOUT_MS = 250;
    const BAR_ASPECT = 2064 / 512;
    const MOBILE_BREAKPOINT = 600;
    const BOOT_SPLASH_MEDIA = '(max-width: 599px)';
    const LEADER_ART_MEDIA = '(orientation: portrait)';

    const LAYOUT = {
        portrait: {
            barWidthRatio: 0.76,
            barSidePadPx: 32,
            bottomRatio: 0.06,
            bottomMinPx: 24,
            textSizePx: 11,
            textNudgePx: -1,
        },
        landscape: {
            barMaxWidth: 480,
            barWidthRatio: 0.42,
            bottomRatio: 0.07,
            bottomMinPx: 32,
            textSizePx: 13,
            textNudgePx: -2,
        },
    };

    function assetPathVariants(file, folder) {
        folder = folder || 'loader';
        const bootBase = window.SOW_BOOT_UI_BASE;
        if (bootBase && typeof bootBase === 'string') {
            const root = bootBase.replace(/\/$/, '');
            if (folder === 'loader') return [root + '/' + file];
            return [root.replace(/\/loader$/, '') + '/' + folder + '/' + file];
        }
        const base = window.SOW_ASSETS_URL;
        if (base && typeof base === 'string') {
            const root = base.replace(/\/$/, '');
            return [root + '/shell/' + folder + '/' + file];
        }
        // Strict endpoints: a missing SOW_ASSETS_URL is a packaging bug —
        // fail loudly instead of guessing a CDN URL (guessing once loaded
        // the wrong origin).
        throw new Error('SOW_ASSETS_URL is not set; cannot resolve boot UI asset: ' + file);
    }

    function wireAssetFallback(img, file) {
        const variants = assetPathVariants(file);
        let attempt = 0;
        img.onerror = function () {
            attempt += 1;
            if (attempt < variants.length) {
                setImgSrc(img, assetUrl(variants[attempt]));
            }
        };
    }

    function assetUrl(path) {
        const v = window.SOW_BUILD_TS;
        if (v && v !== '__BUILD_TS__') {
            return path + '?v=' + encodeURIComponent(v);
        }
        return path;
    }

    function leaderArtUrl(slug, variant) {
        if (!/^[a-z0-9_]+$/.test(String(slug || ''))) return '';
        variant = variant || leaderArtVariant();
        if (variant !== 'desktop' && variant !== 'mobile') return '';
        return assetUrl(assetPathVariants(encodeURIComponent(slug) + '_' + variant + '.webp', 'leaders')[0]);
    }

    function leaderArtVariant() {
        return window.matchMedia
            ? (window.matchMedia(LEADER_ART_MEDIA).matches ? 'mobile' : 'desktop')
            : (window.innerHeight >= window.innerWidth ? 'mobile' : 'desktop');
    }

    function prepareLeaderArt(slug) {
        slug = String(slug || '');
        if (!/^[a-z0-9_]+$/.test(slug)) return Promise.resolve(false);

        const variant = leaderArtVariant();
        const url = leaderArtUrl(slug, variant);
        if (preparedLeaderArt && preparedLeaderArt.url === url) return preparedLeaderArt.promise;

        const image = new Image();
        image.decoding = 'async';
        image.fetchPriority = 'high';
        const prepared = { slug, variant, url, image, ready: false, promise: null };
        preparedLeaderArt = prepared;
        prepared.promise = new Promise((resolve) => {
            let settled = false;
            const finish = (ready) => {
                if (settled) return;
                settled = true;
                image.onload = null;
                image.onerror = null;
                if (preparedLeaderArt === prepared) {
                    if (ready) prepared.ready = true;
                    else preparedLeaderArt = null;
                }
                if (!ready && activeMatchArt === prepared) activeMatchArt = null;
                resolve(ready);
            };
            image.onload = function () {
                if (typeof image.decode !== 'function') return finish(true);
                try {
                    image.decode().then(() => finish(true), () => finish(false));
                } catch (error) {
                    finish(false);
                }
            };
            image.onerror = function () { finish(false); };
            if (isCrossOriginAssetUrl(url)) image.crossOrigin = 'anonymous';
            image.src = url;
        });
        return prepared.promise;
    }

    function localizedLoaderText() {
        if (typeof window.SOW_t === 'function') {
            const value = window.SOW_t('menu.loading');
            if (value && value !== '[menu.loading]') return value;
        }
        return 'Loading...';
    }

    function refreshLoaderText() {
        if (loaderText) loaderText.textContent = localizedLoaderText();
    }

    function isCrossOriginAssetUrl(url) {
        try {
            const resolved = new URL(url, window.location.href);
            return resolved.origin !== window.location.origin;
        } catch {
            return false;
        }
    }

    /** Set src after crossOrigin so CDN boot art can be read in canvas (portal iframe). */
    function setImgSrc(img, url) {
        if (!img || !url) {
            return;
        }
        if (isCrossOriginAssetUrl(url)) {
            img.crossOrigin = 'anonymous';
        } else {
            img.removeAttribute('crossorigin');
        }
        img.src = url;
    }

    function replaceLoaderPicture(media, mobileUrl, preparedImage) {
        const picture = document.createElement('picture');
        picture.className = 'splash-picture';
        let image = preparedImage;
        if (!image) {
            const source = document.createElement('source');
            source.id = 'splash-mobile';
            source.media = media;
            source.srcset = mobileUrl;
            picture.appendChild(source);
            image = document.createElement('img');
        }
        image.id = 'splash-bg';
        image.className = 'splash-bg';
        image.alt = '';
        image.decoding = 'async';
        image.fetchPriority = 'high';
        picture.appendChild(image);

        const previous = document.querySelector('#web-loader .splash-picture');
        if (previous && previous.parentNode) {
            previous.parentNode.replaceChild(picture, previous);
        } else if (root) {
            root.insertBefore(picture, root.firstChild);
        }
        return { picture, image };
    }

    function clearLoaderPicture(key) {
        const picture = document.querySelector('#web-loader .splash-picture');
        if (picture) picture.remove();
        loaderArtKey = key;
    }

    function showBootSplash(key) {
        if (!bootSplashPicture || !root) {
            clearLoaderPicture(key);
            return;
        }
        const current = document.querySelector('#web-loader .splash-picture');
        if (current !== bootSplashPicture) {
            if (current) current.remove();
            root.insertBefore(bootSplashPicture, root.firstChild);
        }
        loaderArtKey = key;
    }

    function applySplashArt() {
        const desktopSplash = assetUrl(assetPathVariants('sow-splash-desktop.webp')[0]);
        const mobileSplash = assetUrl(assetPathVariants('sow-splash-mobile.webp')[0]);
        const { picture, image } = replaceLoaderPicture(BOOT_SPLASH_MEDIA, mobileSplash);
        if (!bootSplashPicture) bootSplashPicture = picture;
        wireAssetFallback(image, isMobile() ? 'sow-splash-mobile.webp' : 'sow-splash-desktop.webp');
        setImgSrc(image, isMobile() ? mobileSplash : desktopSplash);
        loaderArtKey = 'boot';
    }

    function leaderSlugForState(state) {
        const selected = String(state && state.loader_leader || '');
        const leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        const leader = leaders.find((entry) =>
            String(entry && entry.id || '') === selected
            || String(entry && entry.slug || '') === selected,
        );
        const slug = leader && typeof leader.slug === 'string' ? leader.slug : '';
        return /^[a-z0-9_]+$/.test(slug) ? slug : null;
    }

    function waitForExitLeaderArt(prepared, slug) {
        if (pendingExitArt === prepared) return;
        pendingExitArt = prepared;
        prepared.promise.then(() => {
            if (pendingExitArt !== prepared) return;
            pendingExitArt = null;
            const current = latestLoaderState;
            if (!current || current.loader_job !== 'ExitGame' || leaderSlugForState(current) !== slug) return;
            if (prepared.ready && activeMatchArt !== prepared && preparedLeaderArt !== prepared) return;
            const stillPending = syncLoaderArt(current);
            if (current.loader_done === true && !stillPending) finish();
        });
    }

    function syncLoaderArt(state) {
        if (loaderArtKey === 'boot' && state && state.loader_job === 'EnterGame') {
            const slug = leaderSlugForState(state);
            if (slug) {
                prepareLeaderArt(slug);
                if (preparedLeaderArt && preparedLeaderArt.slug === slug) {
                    activeMatchArt = preparedLeaderArt;
                    activeMatchStartedFromBoot = true;
                }
            }
            return null;
        }

        const transition = state && (state.loader_job === 'EnterGame' || state.loader_job === 'ExitGame');
        const hasBootLeader = state && state.loader_job === 'Boot' && state.loader_leader;
        const slug = transition || hasBootLeader ? leaderSlugForState(state) : null;
        if (!slug) {
            if (transition) {
                const key = state.loader_job + ':none';
                if (loaderArtKey !== key) clearLoaderPicture(key);
            } else if (loaderArtKey !== 'boot') {
                applySplashArt();
            }
            return null;
        }

        const key = slug;
        if (state.loader_job === 'EnterGame') {
            activeMatchStartedFromBoot = false;
            if (!activeMatchArt || activeMatchArt.slug !== slug) {
                activeMatchArt = preparedLeaderArt && preparedLeaderArt.slug === slug
                    ? preparedLeaderArt : null;
            }
        }

        if (loaderArtKey === key || loaderArtKey === key + ':failed') return null;

        const prepared = state.loader_job === 'ExitGame' && activeMatchArt && activeMatchArt.slug === slug
            ? activeMatchArt
            : preparedLeaderArt && preparedLeaderArt.slug === slug ? preparedLeaderArt : null;
        if (!prepared) {
            if (loaderArtKey !== key + ':unready') {
                if (state.loader_job === 'ExitGame' && activeMatchStartedFromBoot) showBootSplash(key + ':unready');
                else clearLoaderPicture(key + ':unready');
            }
            return null;
        }
        if (state.loader_job !== 'ExitGame' && prepared.variant !== leaderArtVariant()) {
            if (preparedLeaderArt === prepared) preparedLeaderArt = null;
            if (loaderArtKey !== key + ':unready') clearLoaderPicture(key + ':unready');
            return null;
        }
        if (!prepared.ready) {
            if (loaderArtKey !== key + ':unready') {
                if (state.loader_job === 'ExitGame' && activeMatchStartedFromBoot) showBootSplash(key + ':unready');
                else clearLoaderPicture(key + ':unready');
            }
            if (state.loader_job === 'ExitGame') waitForExitLeaderArt(prepared, slug);
            return state.loader_job === 'ExitGame' ? prepared : null;
        }

        const image = prepared.image;
        const { picture } = replaceLoaderPicture(null, null, image);
        loaderArtKey = key;
        image.onerror = function () {
            if (preparedLeaderArt && preparedLeaderArt.image === image) preparedLeaderArt = null;
            if (activeMatchArt && activeMatchArt.image === image) activeMatchArt = null;
            if (loaderArtKey === key && document.querySelector('#web-loader .splash-picture') === picture) {
                picture.remove();
                loaderArtKey = key + ':failed';
            }
        };
        return null;
    }

    let root = null;
    let barFill = null;
    let barFull = null;
    let loaderText = null;
    let finishing = false;
    let rafId = 0;
    let finishTimer = 0;
    let reportedProgress = null;
    let initialized = false;
    let listenersBound = false;
    let loaderVisible = false;
    let loaderReadyDispatched = false;
    let loaderArtKey = null;
    let preparedLeaderArt = null;
    let activeMatchArt = null;
    let activeMatchStartedFromBoot = false;
    let bootSplashPicture = null;
    let latestLoaderState = null;
    let pendingExitArt = null;

    function isMobile() {
        return window.innerWidth < MOBILE_BREAKPOINT;
    }

    function layoutMode() {
        if (isMobile() && window.innerWidth <= window.innerHeight) {
            return 'portrait';
        }
        return 'landscape';
    }

    function layoutConfig(mode) {
        return LAYOUT[mode];
    }

    function barWidthFor(mode, screenW) {
        const cfg = layoutConfig(mode);
        if (mode === 'portrait') {
            const inner = Math.max(160, screenW - cfg.barSidePadPx * 2);
            return inner * cfg.barWidthRatio;
        }
        return Math.min(cfg.barMaxWidth, screenW * cfg.barWidthRatio);
    }

    function stageSize() {
        const vv = window.visualViewport;
        if (vv && vv.width > 0 && vv.height > 0) {
            return {
                w: vv.width,
                h: vv.height,
                top: vv.offsetTop,
                left: vv.offsetLeft,
            };
        }
        return {
            w: window.innerWidth,
            h: window.innerHeight,
            top: 0,
            left: 0,
        };
    }

    function layout() {
        if (!root) return;
        const mode = layoutMode();
        const cfg = layoutConfig(mode);
        const { w: screenW, h: screenH, top, left } = stageSize();

        root.style.top = top + 'px';
        root.style.left = left + 'px';
        root.style.width = screenW + 'px';
        root.style.height = screenH + 'px';
        const barWidth = barWidthFor(mode, screenW);
        const barHeight = barWidth / BAR_ASPECT;
        const bottomPadding = Math.max(screenH * cfg.bottomRatio, cfg.bottomMinPx);

        root.dataset.layout = mode;

        const barWrapEl = document.getElementById('loader-bar-wrap');
        if (barWrapEl) {
            barWrapEl.style.width = barWidth + 'px';
            barWrapEl.style.height = barHeight + 'px';
            barWrapEl.style.bottom = 'max(' + bottomPadding + 'px, var(--sow-sab))';
        }
        if (barFull) {
            barFull.style.width = barWidth + 'px';
        }
        if (loaderText) {
            loaderText.style.fontSize = cfg.textSizePx + 'px';
            loaderText.style.marginTop = cfg.textNudgePx + 'px';
        }
    }

    function stopProgress() {
        if (rafId) {
            cancelAnimationFrame(rafId);
            rafId = 0;
        }
    }

    /** Slow creep toward 88% while WASM loads; Rust owns completion. */
    function startProgress() {
        if (!barFill) return;
        const t0 = performance.now();
        const durationMs = 2500;

        function tick(now) {
            if (!barFill || finishing) return;
            const t = Math.min(1, (now - t0) / durationMs);
            const eased = 1 - Math.pow(1 - t, 2);
            const progress = reportedProgress == null ? eased * 88 : reportedProgress * 100;
            barFill.style.width = Math.min(100, progress).toFixed(1) + '%';
            rafId = requestAnimationFrame(tick);
        }

        barFill.style.transition = 'none';
        barFill.style.width = '0%';
        reportedProgress = null;
        rafId = requestAnimationFrame(tick);
    }

    function cancelFinishTimers() {
        if (finishTimer) {
            clearTimeout(finishTimer);
            finishTimer = 0;
        }
    }

    function buildDom() {
        root = document.getElementById('web-loader');
        if (!root) {
            root = document.createElement('div');
            root.id = 'web-loader';
            root.setAttribute('aria-live', 'polite');
            root.setAttribute('aria-busy', 'true');
            root.innerHTML = `
                <picture class="splash-picture">
                    <source id="splash-mobile" media="(max-width: 599px)">
                    <img id="splash-bg" class="splash-bg" alt="" decoding="async" fetchpriority="high">
                </picture>
                <div id="loader-bar-wrap" class="loader-bar-wrap">
                    <img id="loader-bar-empty" class="loader-bar-empty" alt="" decoding="async" fetchpriority="high">
                    <div id="loader-bar-fill" class="loader-bar-fill">
                        <img id="loader-bar-full" class="loader-bar-full" alt="" decoding="async" fetchpriority="low">
                    </div>
                    <p id="loader-text" class="loader-text">Loading...</p>
                </div>
            `;
            document.body.appendChild(root);
        }

        barFill = document.getElementById('loader-bar-fill');
        barFull = document.getElementById('loader-bar-full');
        loaderText = document.getElementById('loader-text');

        if (!initialized) {
            const splashImage = document.getElementById('splash-bg');
            if (!splashImage || !splashImage.getAttribute('src')) applySplashArt();
            else loaderArtKey = 'boot';
            if (!bootSplashPicture) bootSplashPicture = document.querySelector('#web-loader .splash-picture');
            setImgSrc(document.getElementById('loader-bar-empty'), assetUrl(assetPathVariants('loader_empty.webp')[0]));
            setImgSrc(document.getElementById('loader-bar-full'), assetUrl(assetPathVariants('loader_full.webp')[0]));
            wireAssetFallback(document.getElementById('loader-bar-empty'), 'loader_empty.webp');
            wireAssetFallback(document.getElementById('loader-bar-full'), 'loader_full.webp');
        }

        layout();
        if (!listenersBound) {
            window.addEventListener('resize', layout);
            window.addEventListener('orientationchange', layout);
            if (window.visualViewport) {
                window.visualViewport.addEventListener('resize', layout);
                window.visualViewport.addEventListener('scroll', layout);
            }
            listenersBound = true;
        }
        if (!initialized) {
            initialized = true;
            loaderVisible = true;
            root.style.visibility = 'visible';
            root.style.opacity = '1';
            root.style.pointerEvents = 'auto';
            root.setAttribute('aria-hidden', 'false');
            startProgress();
        }
    }

    function sowAnalyticsEnvelope(name) {
        let session = window.SOW_ANALYTICS_SESSION_ID;
        if (!session) {
            try { session = sessionStorage.getItem('sow_analytics_session'); } catch (_) {}
            if (!session) {
                session = window.crypto && typeof window.crypto.randomUUID === 'function'
                    ? window.crypto.randomUUID()
                    : String(Date.now()) + Math.random().toString(16).slice(2);
            }
            window.SOW_ANALYTICS_SESSION_ID = session;
            try { sessionStorage.setItem('sow_analytics_session', session); } catch (_) {}
        }
        const userAgent = String(window.navigator.userAgent || '').toLowerCase();
        return {
            v: 1,
            name: name,
            ts_ms: Date.now(),
            session_id: session,
            portal: window.SOW_PORTAL || 'site',
            platform: userAgent.includes('android') ? 'android' : /iphone|ipad|ipod/.test(userAgent) ? 'ios' : 'web',
            build: window.SOW_BUILD_VERSION || 'unknown',
            locale: typeof window.SOW_getLocale === 'function' ? window.SOW_getLocale() : (window.SOW_LOCALE_DEFAULT || 'en'),
            viewport_class: window.innerWidth <= 640 ? 'mobile' : 'desktop',
            experience_cohort: 'unknown',
        };
    }

    function sowTrack(name) {
        if (window.SOW_PORTAL === 'poki') {
            if (typeof window.SOW_pokiMeasure === 'function') {
                window.SOW_pokiMeasure('loading', name, 'complete');
            }
            return;
        }
        if (window.SOW_PORTAL === 'jest') return;
        /* SOW_FIRST_PARTY_ANALYTICS_BEGIN */
        try {
            const base = String(window.SOW_DATABASE_URL || '/api').replace(/\/$/, '');
            fetch(base + '/event', {
                method: 'POST',
                keepalive: true,
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ events: [sowAnalyticsEnvelope(name)] }),
            }).catch(() => {});
        } catch (_) {}
        /* SOW_FIRST_PARTY_ANALYTICS_END */
    }

    function finish() {
        if (!root || finishing || !loaderVisible) return;
        const closingRoot = root;
        finishing = true;
        cancelFinishTimers();
        sowTrack('shell_loaded');
        stopProgress();
        root.style.pointerEvents = 'none';

        if (barFill) {
            barFill.style.transition = 'width 150ms ease-out';
            barFill.style.width = '100%';
        }

        root.style.transition = `opacity ${FADEOUT_MS}ms ease-out`;
        finishTimer = setTimeout(() => {
            finishTimer = 0;
            if (root !== closingRoot) return;
            root.style.opacity = '0';
            root.style.visibility = 'hidden';
            root.style.pointerEvents = 'none';
            root.setAttribute('aria-hidden', 'true');
            root.setAttribute('aria-busy', 'false');
            loaderVisible = false;
            finishing = false;
            clearLoaderPicture(null);
            if (latestLoaderState && latestLoaderState.loader_job === 'ExitGame') {
                activeMatchArt = null;
                activeMatchStartedFromBoot = false;
                pendingExitArt = null;
            }
            if (!loaderReadyDispatched) {
                loaderReadyDispatched = true;
                window.dispatchEvent(new Event('sow:loader-ready'));
            }
            window.dispatchEvent(new Event('sow:loader-cycle-ready'));
        }, 160);
    }

    function sync(state) {
        if (!state) return;
        latestLoaderState = state;
        if (state.loader_job !== 'ExitGame') pendingExitArt = null;
        if (state.loader_done === true && state.loader_job !== 'ExitGame') {
            if (state.loader_job === 'EnterGame') syncLoaderArt(state);
            finish();
            return;
        }

        if (!root) {
            finishing = false;
            buildDom();
        } else if (finishing || !loaderVisible) {
            cancelFinishTimers();
            finishing = false;
            root.style.transition = 'none';
            root.style.opacity = '1';
            root.style.visibility = 'visible';
            root.style.pointerEvents = 'auto';
            root.setAttribute('aria-hidden', 'false');
            loaderVisible = true;
            startProgress();
        }
        const pendingArt = syncLoaderArt(state);
        const progress = Number(state.loader_progress);
        if (Number.isFinite(progress)) {
            const nextProgress = Math.max(0, Math.min(1, progress));
            // Keep the single visible loader moving forward across Boot -> EnterGame.
            reportedProgress = Math.max(reportedProgress == null ? 0 : reportedProgress, nextProgress);
            if (barFill) barFill.style.width = (reportedProgress * 100).toFixed(1) + '%';
        }
        if (loaderText) {
            const status = typeof state.loader_status === 'string' ? state.loader_status.trim() : '';
            loaderText.textContent = status || localizedLoaderText();
        }
        if (root) root.setAttribute('aria-busy', 'true');
        if (state.loader_done === true) {
            if (pendingArt) return;
            finish();
        }
    }

    function initWebLoader() {
        buildDom();
    }

    window.SOW_initWebLoader = initWebLoader;
    window.SOW_refreshWebLoaderText = refreshLoaderText;
    window.SOW_leaderArtUrl = leaderArtUrl;
    window.SOW_prepareLeaderArt = prepareLeaderArt;
    window.addEventListener('sow:locale-change', refreshLoaderText);
    window.SOW_syncWebLoader = sync;

    // Game shell (play subdomain / portal): #web-loader in index.html auto-starts.
    if (document.getElementById('web-loader')) {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', initWebLoader);
        } else {
            initWebLoader();
        }
    }
})();
