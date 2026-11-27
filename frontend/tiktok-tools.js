// Copyright (c) 2026 Benedictus Reynaldo Hartanto (@benedictusrey). All rights reserved.
// TikTok-Now — Desktop Engine & Downloader Integration (v2.2.0)
// Licensed under the MIT License.

(function () {
  'use strict';

  var isTikTokPage = location.hostname === 'tiktok.com' || location.hostname.endsWith('.tiktok.com');

  // ── 0. Immediate Dark Canvas at Document Creation (No White Flash) ───────
  if (isTikTokPage) {
    var forceDarkCSS = document.createElement('style');
    forceDarkCSS.id = 'tiktok-now-dark-canvas';
    forceDarkCSS.textContent = 'html, body { background-color: #0d0e15 !important; color: #ffffff !important; }';
    var appendDarkCanvas = function () {
      var root = document.head || document.documentElement;
      if (root && !document.getElementById('tiktok-now-dark-canvas')) {
        root.appendChild(forceDarkCSS);
      }
      if (document.documentElement) {
        document.documentElement.style.backgroundColor = '#0d0e15';
      }
    };
    if (document.documentElement || document.head) {
      appendDarkCanvas();
    } else {
      var darkCanvasObserver = new MutationObserver(function () {
        if (document.documentElement || document.head) {
          darkCanvasObserver.disconnect();
          appendDarkCanvas();
        }
      });
      darkCanvasObserver.observe(document, { childList: true, subtree: true });
    }
  }

  // ── Network Cache Hook (Early, catches TikTok feed/detail JSON) ──────────
  var _ttnowItemCache = new Map();
  var _TTNOW_CACHE_TTL_MS = 10 * 60 * 1000;
  function cacheTikTokItem(item) {
    if (!item || typeof item !== 'object') return;
    var id = String(item.id || item.aweme_id || (item.itemInfo && item.itemInfo.itemStruct && item.itemInfo.itemStruct.id) || '');
    if (!/^\d{15,22}$/.test(id)) return;
    var payload = '';
    try {
      payload = JSON.stringify(item);
    } catch (_) {
      return;
    }
    if (!payload) return;
    if (_ttnowItemCache.has(id)) _ttnowItemCache.delete(id);
    _ttnowItemCache.set(id, { payload: payload, ts: Date.now() });
    if (_ttnowItemCache.size > 120) {
      var oldest = _ttnowItemCache.keys().next().value;
      if (oldest !== undefined) _ttnowItemCache.delete(oldest);
    }
  }

  function getCachedTikTokItem(id) {
    if (!id) return '';
    var entry = _ttnowItemCache.get(id);
    if (!entry) return '';
    if (Date.now() - entry.ts > _TTNOW_CACHE_TTL_MS) {
      _ttnowItemCache.delete(id);
      return '';
    }
    return entry.payload || '';
  }

  function ingestTikTokJsonText(text) {
    if (!text || text.length < 40 || text.charAt(0) !== '{') return;
    try {
      var data = JSON.parse(text);
      if (!data || typeof data !== 'object') return;
      if (data.itemInfo && data.itemInfo.itemStruct) {
        cacheTikTokItem(data.itemInfo.itemStruct);
      }
      if (Array.isArray(data.itemList)) {
        for (var i = 0; i < data.itemList.length; i++) cacheTikTokItem(data.itemList[i]);
      }
      if (Array.isArray(data.aweme_list)) {
        for (var j = 0; j < data.aweme_list.length; j++) cacheTikTokItem(data.aweme_list[j]);
      }
      if (data.aweme_detail) {
        cacheTikTokItem(data.aweme_detail);
      }
      if (Array.isArray(data.data)) {
        for (var k = 0; k < data.data.length; k++) {
          if (data.data[k] && data.data[k].item) cacheTikTokItem(data.data[k].item);
        }
      }
    } catch (_) {}
  }

  if (isTikTokPage && typeof window.fetch === 'function' && !window.__ttnowFetchHooked) {
    window.__ttnowFetchHooked = true;
    var origFetch = window.fetch;
    window.fetch = function () {
      return origFetch.apply(this, arguments).then(function (res) {
        try {
          var url = (res && res.url) || '';
          if (res && res.ok && /\/api\/(?:post|item|recommend|user|search|related|aweme)/i.test(url)) {
            var clone = res.clone();
            clone.text().then(ingestTikTokJsonText).catch(function () {});
          }
        } catch (_) {}
        return res;
      });
    };
  }

  if (isTikTokPage && typeof window.XMLHttpRequest === 'function' && !window.__ttnowXhrHooked) {
    window.__ttnowXhrHooked = true;
    var origXhrOpen = XMLHttpRequest.prototype.open;
    var origXhrSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function (method, url) {
      this._ttnowUrl = url;
      return origXhrOpen.apply(this, arguments);
    };
    XMLHttpRequest.prototype.send = function () {
      try {
        if (!this._ttnowLoadBound) {
          this._ttnowLoadBound = true;
          this.addEventListener('load', function () {
            try {
              var reqUrl = String(this._ttnowUrl || this.responseURL || '');
              if (this.status >= 200 && this.status < 300 &&
                  /\/api\/(?:post|item|recommend|user|search|related|aweme)/i.test(reqUrl) &&
                  (!this.responseType || this.responseType === 'text')) {
                ingestTikTokJsonText(this.responseText);
              }
            } catch (_) {}
          });
        }
      } catch (_) {}
      return origXhrSend.apply(this, arguments);
    };
  }

  function start() {
    try {
      window.__tiktoknow_is_dm = function () {
        return location.pathname === '/messages' || location.pathname.indexOf('/messages/') === 0;
      };

      function getInvoke() {
        if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
          return function (cmd, args) { return window.__TAURI_INTERNALS__.invoke(cmd, args); };
        }
        if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
          return function (cmd, args) { return window.__TAURI__.core.invoke(cmd, args); };
        }
        return null;
      }

      function nativeInvoke(cmd, args) {
        var fn = getInvoke();
        if (!fn) return Promise.reject(new Error('TikTok-Now native bridge unavailable'));
        return fn(cmd, args);
      }

      // ── Platform & Motion Helpers ────────────────────────────────────────
      function isLinuxWebKitRuntime() {
        return /Linux/i.test(String((navigator && navigator.platform) || ''))
          && !/(?:Chrome|Chromium|Edg)\//i.test(String((navigator && navigator.userAgent) || ''));
      }

      function prefersReducedMotion() {
        return typeof window.matchMedia === 'function'
          && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      }

      function isWindowForeground() {
        return !document.hidden && (typeof document.hasFocus !== 'function' || document.hasFocus());
      }

      // ── Handle Extraction ────────────────────────────────────────────────
      function getTikTokUserHandle() {
        try {
          try {
            var uData = window.__UNIVERSAL_DATA_FOR_REHYDRATION__;
            if (uData && uData.__DEFAULT_SCOPE__) {
              var s = uData.__DEFAULT_SCOPE__;
              if (s['webapp.app-context'] && s['webapp.app-context'].user && s['webapp.app-context'].user.uniqueId) {
                var u1 = s['webapp.app-context'].user.uniqueId;
                if (u1 && !/^\d+$/.test(u1)) return u1;
              }
              if (s['webapp.user-detail'] && s['webapp.user-detail'].userInfo && s['webapp.user-detail'].userInfo.user && s['webapp.user-detail'].userInfo.user.uniqueId) {
                var u2 = s['webapp.user-detail'].userInfo.user.uniqueId;
                if (u2 && !/^\d+$/.test(u2)) return u2;
              }
            }
          } catch (_) {}

          try {
            if (window.SIGI_STATE && window.SIGI_STATE.AppContext && window.SIGI_STATE.AppContext.user && window.SIGI_STATE.AppContext.user.uniqueId) {
              var u3 = window.SIGI_STATE.AppContext.user.uniqueId;
              if (u3 && !/^\d+$/.test(u3)) return u3;
            }
          } catch (_) {}

          try {
            if (window.__INITIAL_STATE__ && window.__INITIAL_STATE__.appContext && window.__INITIAL_STATE__.appContext.user && window.__INITIAL_STATE__.appContext.user.uniqueId) {
              var u4 = window.__INITIAL_STATE__.appContext.user.uniqueId;
              if (u4 && !/^\d+$/.test(u4)) return u4;
            }
          } catch (_) {}

          var navProfile = document.querySelector('a[data-e2e="nav-profile"]')
            || document.querySelector('a[data-e2e="profile-icon"]')
            || document.querySelector('a[data-e2e="user-profile"]')
            || document.querySelector('a[href^="/@"][class*="SideNav"]')
            || document.querySelector('[class*="HeaderRight"] a[href*="/@"]')
            || document.querySelector('[class*="DivHeaderRight"] a[href*="/@"]');
          if (navProfile) {
            var href = navProfile.getAttribute('href') || '';
            var m = href.match(/\/@([a-zA-Z0-9_.]+)/);
            if (m && m[1]) {
              var handle = m[1].replace(/\/$/, '');
              if (!/^\d+$/.test(handle)) return handle;
            }
          }

          if (location.pathname.indexOf('/@') === 0) {
            var isMyProfile = document.querySelector('[data-e2e="edit-profile-entrance"]')
              || Array.from(document.querySelectorAll('button')).some(function (b) {
                return b.textContent && b.textContent.indexOf('Edit profile') !== -1;
              });
            if (isMyProfile) {
              var pathHandle = location.pathname.split('/@')[1].split('/')[0];
              if (pathHandle && !/^\d+$/.test(pathHandle)) return pathHandle;
            }
          }
        } catch (_) {}
        return null;
      }

      // ── 0b. Targeted Auth Toast Detection (No innerText Reflow Jank!) ────
      function checkAuthToastNodes(root) {
        if (!root || !root.querySelectorAll) return false;
        var candidates = root.querySelectorAll('[role="alert"], [role="status"], [class*="Toast"], [class*="toast"], [class*="Notice"], [data-e2e*="modal"], [class*="Modal"], [role="dialog"]');
        for (var i = 0; i < candidates.length; i++) {
          var txt = candidates[i].textContent || '';
          if (txt.length < 160 && (txt.indexOf('Logged in') !== -1 || txt.indexOf('Login success') !== -1)) {
            return true;
          }
        }
        return false;
      }

      // ── 0c. Captcha & Slider Dragging Fix ────────────────────────────────
      document.addEventListener('dragstart', function (e) {
        if (!e.target || !e.target.closest) return;
        if (e.target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]')) return;
        var tag = e.target.tagName ? e.target.tagName.toUpperCase() : '';
        if (tag === 'IMG' || tag === 'SVG'
          || e.target.closest('[class*="captcha"]')
          || e.target.closest('[class*="sec-captcha"]')
          || e.target.closest('[class*="slider"]')
          || e.target.closest('[class*="verify"]')
          || e.target.closest('[class*="puzzle"]')) {
          e.preventDefault();
        }
      }, true);

      var styleParent = document.head || document.documentElement;
      if (styleParent && !document.getElementById('tiktok-now-captcha-fix')) {
        var captchaCSS = document.createElement('style');
        captchaCSS.id = 'tiktok-now-captcha-fix';
        captchaCSS.textContent =
          'input, textarea, select, [contenteditable=""], [contenteditable="true"] {\n' +
          '  -webkit-user-select: text !important;\n' +
          '  user-select: text !important;\n' +
          '  touch-action: auto !important;\n' +
          '  -webkit-user-drag: auto !important;\n' +
          '}\n' +
          '[class*="captcha"]:not(input):not(textarea):not(select),\n' +
          '[class*="sec-captcha"]:not(input):not(textarea):not(select),\n' +
          '[class*="slider"]:not(input):not(textarea):not(select),\n' +
          '[class*="verify"]:not(input):not(textarea):not(select),\n' +
          '[class*="puzzle"]:not(input):not(textarea):not(select) {\n' +
          '  -webkit-user-drag: none !important;\n' +
          '  user-select: none !important;\n' +
          '  -webkit-user-select: none !important;\n' +
          '  touch-action: none !important;\n' +
          '}\n' +
          '[class*="captcha"] img, [class*="sec-captcha"] img, [class*="captcha"] svg, [class*="sec-captcha"] svg {\n' +
          '  -webkit-user-drag: none !important;\n' +
          '  pointer-events: none !important;\n' +
          '}\n';
        styleParent.appendChild(captchaCSS);
      }

      window.autoScrollEnabled = false;
      window.__tiktoknow_typing = false;

      // ── 1. Bottom-Center Floating Toast Notification ─────────────────────
      function showToast(msg) {
        var root = document.body || document.documentElement;
        if (!root) return;
        var t = document.getElementById('tiktok-now-toast');
        if (!t) {
          t = document.createElement('div');
          t.id = 'tiktok-now-toast';
          t.style.cssText =
            'position: fixed;' +
            'bottom: 28px;' +
            'left: 50%;' +
            'transform: translateX(-50%) translateY(8px);' +
            'opacity: 0;' +
            'background: rgba(13, 14, 21, 0.94);' +
            'color: #00F2FE;' +
            'border: 1px solid rgba(255, 0, 127, 0.72);' +
            'padding: 10px 20px;' +
            'border-radius: 9999px;' +
            'font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;' +
            'font-weight: 600;' +
            'font-size: 13px;' +
            'z-index: 2147483647;' +
            'box-shadow: 0 10px 26px rgba(0, 0, 0, 0.72);' +
            'transition: ' + (prefersReducedMotion() ? 'none' : 'opacity 0.22s ease, transform 0.22s ease') + ';' +
            'pointer-events: none;' +
            'white-space: nowrap;' +
            'max-width: 86vw;' +
            'overflow: hidden;' +
            'text-overflow: ellipsis;';
          root.appendChild(t);
        }
        t.textContent = msg;
        t.style.opacity = '1';
        t.style.transform = 'translateX(-50%) translateY(0)';
        clearTimeout(window._toastTimeout);
        window._toastTimeout = setTimeout(function () {
          t.style.opacity = '0';
          t.style.transform = 'translateX(-50%) translateY(8px)';
        }, 2200);
      }
      window.showToast = showToast;

      window.toggleAutoScroll = function () {
        if (window.__tiktoknow_is_dm()) {
          showToast('Auto-Scroll is not available in Messages');
          return;
        }
        window.autoScrollEnabled = !window.autoScrollEnabled;
        showToast(window.autoScrollEnabled ? 'Auto-Scroll: ENABLED' : 'Auto-Scroll: DISABLED');
      };

      // ── 2. Desktop Styles, Double-Click Selection Fix & Stage Button CSS ──
      if (styleParent && !document.getElementById('tiktok-now-desktop-styles')) {
        var style = document.createElement('style');
        style.id = 'tiktok-now-desktop-styles';
        style.textContent =
          '[class*="DivBannerContainer"],\n' +
          '[class*="DivAppDownload"],\n' +
          '[class*="ButtonGetApp"],\n' +
          '[data-e2e="open-app-btn"],\n' +
          '.tiktok-banner-container,\n' +
          'div[class*="tiktok-download"] {\n' +
          '  display: none !important;\n' +
          '}\n' +
          '/* Prevent double-click-to-like blue selection highlight on video/player surfaces */\n' +
          'video,\n' +
          '[class*="DivPlayerContainer"],\n' +
          '[class*="DivVideoWrapper"],\n' +
          '[class*="DivBasicPlayerWrapper"],\n' +
          '[data-e2e="feed-video"] {\n' +
          '  -webkit-user-select: none !important;\n' +
          '  user-select: none !important;\n' +
          '}\n' +
          '/* Keep comments, captions, descriptions, and DMs selectable */\n' +
          '[data-e2e*="comment"], [data-e2e*="desc"],\n' +
          '[class*="DivCommentContentWrapper"], [class*="DivMessage"],\n' +
          '[class*="CommentItem"], [class*="VideoDesc"] {\n' +
          '  -webkit-user-select: text !important;\n' +
          '  user-select: text !important;\n' +
          '}\n' +
          '/* Video mount guard: avoid black flash frame while <video> decodes first frame */\n' +
          '[data-ttnow-mount-wait="1"] {\n' +
          '  visibility: hidden !important;\n' +
          '}\n' +
          '/* Stage-anchored Download button (stays inside video/photo stage left of Comments) */\n' +
          '.ttnow-media-host {\n' +
          '  position: relative !important;\n' +
          '}\n' +
          '.ttnow-download-btn {\n' +
          '  position: absolute !important;\n' +
          '  top: 12px !important;\n' +
          '  right: 12px !important;\n' +
          '  z-index: 2147483646 !important;\n' +
          '  width: 36px !important;\n' +
          '  height: 36px !important;\n' +
          '  min-width: 36px !important;\n' +
          '  min-height: 36px !important;\n' +
          '  padding: 0 !important;\n' +
          '  margin: 0 !important;\n' +
          '  border-radius: 9999px !important;\n' +
          '  border: 1px solid rgba(255, 255, 255, 0.22) !important;\n' +
          '  background: rgba(13, 14, 21, 0.86) !important;\n' +
          '  color: #f4f5f8 !important;\n' +
          '  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.45) !important;\n' +
          '  display: inline-flex !important;\n' +
          '  align-items: center !important;\n' +
          '  justify-content: center !important;\n' +
          '  cursor: pointer !important;\n' +
          '  opacity: 0.9 !important;\n' +
          '  transition: background-color 140ms ease, border-color 140ms ease, opacity 140ms ease, transform 100ms ease !important;\n' +
          '  pointer-events: auto !important;\n' +
          '}\n' +
          '.ttnow-download-btn--shifted {\n' +
          '  right: 56px !important;\n' +
          '}\n' +
          '.ttnow-download-btn:hover {\n' +
          '  background: rgba(24, 27, 40, 0.96) !important;\n' +
          '  border-color: rgba(0, 242, 254, 0.65) !important;\n' +
          '  color: #00f2fe !important;\n' +
          '  opacity: 1 !important;\n' +
          '}\n' +
          '.ttnow-download-btn:active {\n' +
          '  transform: scale(0.95) !important;\n' +
          '}\n' +
          '.ttnow-download-btn:focus-visible {\n' +
          '  outline: 2px solid #00f2fe !important;\n' +
          '  outline-offset: 2px !important;\n' +
          '}\n' +
          '@media (prefers-reduced-motion: reduce) {\n' +
          '  .ttnow-download-btn { transition: none !important; }\n' +
          '  .ttnow-download-btn:active { transform: none !important; }\n' +
          '}\n' +
          '::-webkit-scrollbar { width: 8px; height: 8px; }\n' +
          '::-webkit-scrollbar-track { background: #0d0e15; }\n' +
          '::-webkit-scrollbar-thumb { background: #00F2FE; border-radius: 4px; }\n';
        styleParent.appendChild(style);
      }

      // Linux WebKitGTK CPU SkBlurEngine mitigation
      if (isLinuxWebKitRuntime() && styleParent && !document.getElementById('ttnow-linux-perf')) {
        var linuxPerfStyle = document.createElement('style');
        linuxPerfStyle.id = 'ttnow-linux-perf';
        linuxPerfStyle.textContent =
          '[class*="Blur"], [class*="Backdrop"], [class*="DivBlur"], [class*="Modal"] {\n' +
          '  backdrop-filter: none !important;\n' +
          '  -webkit-backdrop-filter: none !important;\n' +
          '}\n' +
          'video ~ img, [class*="DivPlayerContainer"] img[class*="Blur"] {\n' +
          '  filter: none !important;\n' +
          '  -webkit-filter: none !important;\n' +
          '}\n';
        styleParent.appendChild(linuxPerfStyle);
      }

      // Capture selectstart + dblclick selection cleanup for clean double-click-to-like
      var PLAYER_SEL = 'video, [class*="DivPlayerContainer"], [class*="DivVideoWrapper"], [class*="DivBasicPlayerWrapper"], [data-e2e="feed-video"]';
      var TEXT_EXEMPT_SEL = 'input, textarea, select, [contenteditable=""], [contenteditable="true"], [data-e2e*="comment"], [data-e2e*="desc"], [class*="Comment"], [class*="Caption"], [class*="Description"], [class*="Message"]';

      document.addEventListener('selectstart', function (e) {
        if (window.__tiktoknow_is_dm()) return;
        var t = e.target;
        if (!t || !t.closest) return;
        if (t.closest(TEXT_EXEMPT_SEL)) return;
        if (t.tagName === 'VIDEO' || t.closest(PLAYER_SEL)) {
          e.preventDefault();
        }
      }, true);

      document.addEventListener('dblclick', function (e) {
        if (window.__tiktoknow_is_dm()) return;
        var t = e.target;
        if (!t || !t.closest) return;
        if (t.closest(TEXT_EXEMPT_SEL) || t.closest('.ttnow-download-btn')) return;
        if (t.tagName === 'VIDEO' || t.closest(PLAYER_SEL)) {
          try {
            var sel = window.getSelection();
            if (sel) sel.removeAllRanges();
          } catch (_) {}
        }
      }, true);

      // ── 3. Smart Scroll Targeting ────────────────────────────────────────
      function findScrollContainer() {
        var candidates = document.querySelectorAll('main, [id*="main-content"], [class*="DivFeed"], [class*="DivList"], [class*="ColumnList"], [data-e2e="recommend-list-item-container"]');
        var best = null;
        var bestHeight = 0;
        for (var i = 0; i < candidates.length; i++) {
          var el = candidates[i];
          var cur = el;
          for (var hop = 0; cur && cur !== document.body && cur !== document.documentElement && hop < 5; hop++, cur = cur.parentElement) {
            try {
              var cs = getComputedStyle(cur);
              if ((cs.overflowY === 'auto' || cs.overflowY === 'scroll') && cur.scrollHeight > cur.clientHeight + 40) {
                if (cur.clientHeight > bestHeight) {
                  bestHeight = cur.clientHeight;
                  best = cur;
                }
              }
            } catch (_) {}
          }
        }
        return best;
      }

      window.scrollFeedBy = function (deltaY) {
        var behavior = prefersReducedMotion() ? 'auto' : 'smooth';
        var container = findScrollContainer();
        if (container && typeof container.scrollBy === 'function') {
          var before = container.scrollTop;
          container.scrollBy({ top: deltaY, behavior: behavior });
          if (container.scrollHeight > container.clientHeight + 20) return;
          if (container.scrollTop !== before) return;
        }
        window.scrollBy({ top: deltaY, behavior: behavior });
      };

      // ── 4. Video Mount Guard + Observer + Non-Fighting Audio Defaults ────
      var MOUNT_GUARD_ATTR = 'data-ttnow-mount-wait';
      var MOUNT_GUARD_TIMEOUT_MS = 1500;
      var activeVideo = null;
      var _intendedPlaying = false;
      var _resumeOnVisible = false;
      var _wasMutedBeforeHide = false;
      var _mutedByHide = false;
      var _playingAudios = [];
      var _userTouchedAudio = false;
      var _startupGraceUntil = Date.now() + 60000;
      var _programmaticAudio = false;

      function isDarkSurface(color) {
        var m = color && color.match(/^rgb\((\d+), (\d+), (\d+)\)$/);
        if (!m) return false;
        return (0.299 * m[1] + 0.587 * m[2] + 0.114 * m[3]) <= 40;
      }

      function mountGuardRelease(el) {
        if (el && el.getAttribute && el.getAttribute(MOUNT_GUARD_ATTR) !== null) {
          el.removeAttribute(MOUNT_GUARD_ATTR);
        }
      }

      function mountGuardCleanup(video) {
        if (!video) return;
        if (video.__ttnowGuardTimer) {
          clearTimeout(video.__ttnowGuardTimer);
          video.__ttnowGuardTimer = 0;
        }
        if (video.__ttnowGuardAbort) {
          video.__ttnowGuardAbort.abort();
          video.__ttnowGuardAbort = null;
        }
      }

      function mountGuardReleaseTree(video) {
        mountGuardCleanup(video);
        mountGuardRelease(video);
        if (video && video.__ttnowGuardHost) {
          mountGuardRelease(video.__ttnowGuardHost);
          video.__ttnowGuardHost = null;
        }
      }

      function mountGuardMark(video, force) {
        if (!(video instanceof HTMLVideoElement)) return;
        if (video.readyState >= 2) {
          video.__ttnowGuardTimedOut = false;
          mountGuardReleaseTree(video);
          return;
        }
        var curSrc = video.currentSrc || video.src || '';
        if (video.__ttnowGuardTimedOut && curSrc && curSrc === video.__ttnowGuardSrc) return;
        if (video.getAttribute(MOUNT_GUARD_ATTR) !== null) {
          if (!force || (curSrc && curSrc === video.__ttnowGuardSrc)) return;
        }
        mountGuardCleanup(video);
        var gen = (video.__ttnowGuardGen || 0) + 1;
        video.__ttnowGuardGen = gen;
        video.__ttnowGuardSrc = curSrc;
        video.__ttnowGuardTimedOut = false;
        video.setAttribute(MOUNT_GUARD_ATTR, '1');
        // Never hide DivContainer (holds TikTok's poster <img>); only hide an immediate
        // dark xgplayer wrapper if it has no poster <img> inside it.
        if (!video.__ttnowGuardHost) {
          try {
            var p = video.parentElement;
            if (p && !p.querySelector('img') &&
                (p.tagName === 'XG-VIDEO-CONTAINER' || (typeof p.className === 'string' && p.className.indexOf('xgplayer') !== -1)) &&
                isDarkSurface(getComputedStyle(p).backgroundColor)) {
              p.setAttribute(MOUNT_GUARD_ATTR, '1');
              video.__ttnowGuardHost = p;
            }
          } catch (_) {}
        }
        var release = function () {
          if (video.__ttnowGuardGen !== gen) return;
          video.__ttnowGuardTimedOut = false;
          mountGuardReleaseTree(video);
        };
        var ac = typeof AbortController === 'function' ? new AbortController() : null;
        video.__ttnowGuardAbort = ac;
        var opts = ac ? { once: true, signal: ac.signal } : { once: true };
        video.addEventListener('loadeddata', release, opts);
        video.addEventListener('canplay', release, opts);
        video.addEventListener('playing', release, opts);
        video.__ttnowGuardTimer = setTimeout(function () {
          if (video.__ttnowGuardGen !== gen) return;
          video.__ttnowGuardTimedOut = true;
          mountGuardReleaseTree(video);
        }, MOUNT_GUARD_TIMEOUT_MS);
      }

      ['loadstart', 'emptied'].forEach(function (type) {
        document.addEventListener(type, function (ev) {
          if (!(ev.target instanceof HTMLVideoElement)) return;
          if (type === 'emptied') {
            ev.target.__ttnowGuardTimedOut = false;
            ev.target.__ttnowGuardSrc = '';
          }
          mountGuardMark(ev.target, true);
        }, true);
      });

      function maybeStartupUnmute(v) {
        if (!v || _mutedByHide || document.hidden) return;
        if (_userTouchedAudio || v.dataset.ttnowUserAudio === '1') return;
        if (Date.now() > _startupGraceUntil && v.dataset.ttnowAudioInitialized === '1') return;
        v.dataset.ttnowAudioInitialized = '1';
        _programmaticAudio = true;
        try {
          if (v.muted) v.muted = false;
          if (Math.abs((v.volume || 0) - 0.5) > 0.01 && v.volume === 1) {
            v.volume = 0.5;
          } else if (v.volume === 0) {
            v.volume = 0.5;
          }
        } finally {
          setTimeout(function () { _programmaticAudio = false; }, 0);
        }
      }

      window.__tiktoknowStartupTick = function () {
        maybeStartupUnmute(activeVideo || document.querySelector('video'));
      };

      window.__tiktoknowActiveVideo = function () {
        if (activeVideo && activeVideo.isConnected) return activeVideo;
        var playing = Array.from(document.querySelectorAll('video')).find(function (v) { return !v.paused; });
        return playing || document.querySelector('video') || null;
      };

      window.__tiktoknowEnsureForegroundAudio = function () {
        try {
          if (!isWindowForeground()) return 'hidden';
          var v = window.__tiktoknowActiveVideo();
          if (!v) return 'no-video';
          if (_mutedByHide) {
            _programmaticAudio = true;
            try { v.muted = _wasMutedBeforeHide; } finally { _programmaticAudio = false; }
            _mutedByHide = false;
          }
          if (!_userTouchedAudio && v.dataset.ttnowUserAudio !== '1' && !v.paused && v.muted) {
            _programmaticAudio = true;
            try {
              v.muted = false;
              if (v.volume <= 0.005) v.volume = 0.5;
            } finally {
              _programmaticAudio = false;
            }
            return 'unmuted';
          }
          return 'ok';
        } catch (_) {
          return 'error';
        }
      };

      var observer = new IntersectionObserver(function (entries) {
        entries.forEach(function (entry) {
          var vid = entry.target;
          if (entry.isIntersecting && entry.intersectionRatio >= 0.5) {
            if (activeVideo && activeVideo !== vid) {
              activeVideo.pause();
            }
            vid.play().catch(function () {});
            activeVideo = vid;
            _intendedPlaying = true;
            maybeStartupUnmute(vid);
          } else if (!entry.isIntersecting && activeVideo === vid) {
            vid.pause();
            activeVideo = null;
            _intendedPlaying = false;
          }
        });
      }, { threshold: [0.5] });

      function setupVideoListeners(v) {
        if (!(v instanceof HTMLVideoElement)) return;
        mountGuardMark(v, false);
        observer.observe(v);
        if (!v.dataset.hasEndedListener) {
          v.dataset.hasEndedListener = 'true';
          v.addEventListener('ended', function () {
            if (window.autoScrollEnabled && !window.__tiktoknow_is_dm()) {
              window.scrollFeedBy(window.innerHeight * 0.85);
            }
          });
          v.addEventListener('play', function () {
            activeVideo = v;
            maybeStartupUnmute(v);
          });
          v.addEventListener('volumechange', function () {
            if (!_programmaticAudio && !_mutedByHide && !document.hidden) {
              _userTouchedAudio = true;
              v.dataset.ttnowUserAudio = '1';
            }
          });
        }
      }

      // ── 5. Stage-Anchored Top-Right Download Button ──────────────────────
      function isPostVideo(video) {
        if (!(video instanceof HTMLVideoElement) || !video.isConnected) return false;
        if (video.closest('.ttnow-download-btn, #tiktok-now-toast, #__tiktoknow_about')) return false;
        var cs = getComputedStyle(video);
        if (cs.display === 'none') return false;
        var r = video.getBoundingClientRect();
        return r.width >= 180 && r.height >= 150;
      }

      function isPostImage(img) {
        if (!(img instanceof HTMLImageElement) || !img.isConnected) return false;
        if (img.getAttribute('aria-hidden') === 'true' || (img.src && img.src.indexOf('data:') === 0)) return false;
        if (img.closest('header, nav, [class*="Comment"], [class*="SideNav"], [class*="Avatar"], [data-e2e*="avatar"], .ttnow-download-btn, #__tiktoknow_about')) return false;
        if (!img.closest('main, [role="dialog"], [class*="DivBrowserModeContainer"]')) return false;
        var nw = img.naturalWidth || 0;
        var nh = img.naturalHeight || 0;
        if (nw > 0 && (nw < 160 || nh < 160)) return false;
        var cs = getComputedStyle(img);
        if (cs.display === 'none' || cs.visibility === 'hidden') return false;
        var r = img.getBoundingClientRect();
        return r.width >= 180 && r.height >= 150;
      }

      var OUTER_VIDEO_STAGE_SEL = '[class*="BasePlayerContainer"], [class*="DivVideoPlayerContainer"], section[data-e2e="feed-video"], [data-e2e="feed-video"], [class*="DivPlayerContainer"], [class*="DivHeroCover"]';

      function findVideoStageHost(video) {
        if (!isPostVideo(video)) return null;
        var vr = video.getBoundingClientRect();
        var stage = video.parentElement;
        var fallback = null;
        for (var hop = 0; stage && hop < 8; hop++, stage = stage.parentElement) {
          if (stage.matches('body, html, main, article')) break;
          if (stage.matches('[class*="DivComment"], [class*="CommentList"], [class*="DivBrowserModeContainer"]')) break;
          if (stage.matches('section') && !stage.matches('[data-e2e="feed-video"]')) break;
          var sr = stage.getBoundingClientRect();
          var visOk = sr.width >= 180 && sr.height >= 150 && getComputedStyle(stage).visibility !== 'hidden';
          if (visOk && stage.matches(OUTER_VIDEO_STAGE_SEL)) {
            // If DivMediaCardOverlay is a sibling inside an outer [data-e2e="feed-video"] section,
            // climb one step higher so the button sits above DivMediaCardOverlay.
            var parentFeed = stage.parentElement && stage.parentElement.matches('[data-e2e="feed-video"]');
            if (parentFeed && !stage.querySelector('[class*="DivMediaCardOverlay"]') &&
                stage.parentElement.querySelector('[class*="DivMediaCardOverlay"]')) {
              fallback = stage;
              continue;
            }
            return stage;
          }
          if (visOk) {
            if (stage.matches('[class*="DivVideoWrapper"], [class*="DivBasicPlayerWrapper"]')) {
              fallback = stage;
            } else if (!fallback) {
              fallback = stage;
            }
          }
          var parent = stage.parentElement;
          if (!parent || parent.matches('body, html, main, article')) break;
          if (parent.matches('section') && !parent.matches('[data-e2e="feed-video"]')) break;
          var pr = parent.getBoundingClientRect();
          // Allow wide letterboxed stages (e.g. xgplayer-container / DivBasicPlayerWrapper) when height aligns with video
          var heightAligned = pr.height > 0 &&
            (Math.abs(pr.height - vr.height) <= 120 ||
             (pr.height >= vr.height * 0.9 && pr.height <= vr.height * 1.5 + 120));
          if (pr.height > 0 && pr.width > vr.width * 1.25 + 64 && !heightAligned) break;
        }
        return fallback;
      }

      function hasVideoInImageAncestry(img, ir) {
        var videoCard = img.closest('[class*="DivVideoPlayerContainer"], [class*="BasePlayerContainer"], [class*="DivBasicPlayerWrapper"], [data-e2e="feed-video"]');
        if (videoCard) {
          var cardVideos = videoCard.querySelectorAll('video');
          for (var i = 0; i < cardVideos.length; i++) {
            if (isPostVideo(cardVideos[i])) return true;
          }
        }
        var sec = img.closest('section, article');
        if (sec) {
          var secVideos = sec.querySelectorAll('video');
          for (var j = 0; j < secVideos.length; j++) {
            if (!isPostVideo(secVideos[j])) continue;
            var vr = secVideos[j].getBoundingClientRect();
            if (vr.right > ir.left + 20 && vr.left < ir.right - 20 &&
                vr.bottom > ir.top + 20 && vr.top < ir.bottom - 20) {
              return true;
            }
          }
        }
        return false;
      }

      function findImageStageHost(image) {
        if (!isPostImage(image)) return null;
        if (image.closest('.swiper-slide-duplicate')) return null;
        var ir = image.getBoundingClientRect();
        if (hasVideoInImageAncestry(image, ir)) return null;
        var stage = image.closest('ul') ? image.closest('ul').parentElement : image.parentElement;
        while (stage && stage.matches('picture, a, li, ul, span')) {
          var p = stage.parentElement;
          if (!p || p.matches('body, html, main, section, article')) break;
          stage = p;
        }
        var fallback = null;
        for (var hop = 0; stage && hop < 8; hop++, stage = stage.parentElement) {
          if (stage.matches('body, html, main, article, [class*="DivComment"], [class*="DivBrowserModeContainer"]')) break;
          if (stage.matches('section') && !stage.matches('[data-e2e="feed-video"], [class*="SectionMediaCardContainer"]')) break;
          if (stage.matches('picture, span')) continue;
          var sr = stage.getBoundingClientRect();
          var visOk = sr.width >= 180 && sr.height >= 150 && getComputedStyle(stage).visibility !== 'hidden';
          if (visOk && stage.matches(OUTER_VIDEO_STAGE_SEL + ', [class*="SectionMediaCardContainer"]')) {
            var parentStage = stage.parentElement && stage.parentElement.matches('[data-e2e="feed-video"], [class*="SectionMediaCardContainer"]');
            if (parentStage && !stage.querySelector('[class*="DivMediaCardOverlay"]') &&
                stage.parentElement.querySelector('[class*="DivMediaCardOverlay"]')) {
              fallback = stage;
              continue;
            }
            return stage;
          }
          var inSwiper = stage.matches('.swiper-slide, .swiper-wrapper, .swiper, [class*="DivSwiperContainer"], [class*="DivPhotoVideoContainer"]');
          if (!inSwiper && (stage.scrollWidth > stage.clientWidth + 8 || sr.width > ir.width * 1.25)) {
            if (!stage.parentElement || stage.parentElement.matches('body, html, main, section, article')) break;
            continue;
          }
          var vids = stage.querySelectorAll('video');
          for (var v = 0; v < vids.length; v++) {
            if (isPostVideo(vids[v])) return null;
          }
          if (getComputedStyle(stage).visibility === 'hidden') continue;
          if (!inSwiper && visOk && !fallback) fallback = stage;
        }
        return fallback;
      }

      function hasTopRightNativeButton(stage, stageRect) {
        var sr = stageRect || stage.getBoundingClientRect();
        if (sr.width <= 0 || sr.height <= 0) return false;
        var btns = stage.querySelectorAll('button, [role="button"], [class*="DivMediaCardOverlayTopActions"] > *');
        for (var i = 0; i < btns.length; i++) {
          var el = btns[i];
          if (el.closest('.ttnow-download-btn')) continue;
          if (el.matches('[data-e2e="arrow-next"], [data-e2e="arrow-prev"]')) continue;
          if (getComputedStyle(el).visibility === 'hidden') continue;
          var r = el.getBoundingClientRect();
          if (r.width > 0 && r.width <= 120 && r.height > 0 && r.height <= 80 &&
              r.right >= sr.right - 64 && r.top <= sr.top + 64) {
            return true;
          }
        }
        return false;
      }

      var _stageTargetMedia = new WeakMap();
      function mountStageDownloadButton(stage, targetMedia) {
        if (!stage || !stage.isConnected) return;
        if (window.__tiktoknow_is_dm()) return;
        var sr = stage.getBoundingClientRect();
        if (sr.width < 180 || sr.height < 150 || getComputedStyle(stage).visibility === 'hidden') return;
        // Only apply position: relative when computed position is static; never override
        // positioned containers (e.g. absolute/relative DivBasicPlayerWrapper or stage wrappers).
        if (!stage.classList.contains('ttnow-media-host') && getComputedStyle(stage).position === 'static') {
          stage.classList.add('ttnow-media-host');
        }
        stage.setAttribute('data-ttnow-stage-host', '1');
        var btn = null;
        for (var i = 0; i < stage.children.length; i++) {
          if (stage.children[i].classList && stage.children[i].classList.contains('ttnow-download-btn')) {
            btn = stage.children[i];
            break;
          }
        }
        if (!btn) {
          btn = document.createElement('button');
          btn.type = 'button';
          btn.className = 'ttnow-download-btn';
          btn.setAttribute('aria-label', 'Open TikTok-Now Downloader');
          btn.title = 'Open TikTok-Now Downloader';
          btn.innerHTML =
            '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
            '<path d="M12 3v12 m-5-5 5 5 5-5 M5 20h14"/></svg>';
          btn.addEventListener('contextmenu', function (ev) {
            ev.preventDefault();
            ev.stopPropagation();
          });
          stage.appendChild(btn);
        }
        btn.classList.toggle('ttnow-download-btn--shifted', hasTopRightNativeButton(stage, sr));
        _stageTargetMedia.set(btn, { stage: stage, media: targetMedia });
      }

      function scanStageButtons() {
        if (!isTikTokPage || window.__tiktoknow_is_dm()) return;
        var videos = document.querySelectorAll('video');
        for (var i = 0; i < videos.length; i++) {
          setupVideoListeners(videos[i]);
          var vStage = findVideoStageHost(videos[i]);
          if (vStage) mountStageDownloadButton(vStage, videos[i]);
        }
        var imgs = document.querySelectorAll('img');
        for (var j = 0; j < imgs.length; j++) {
          var im = imgs[j];
          if (im.closest('header, nav, [class*="Comment"], [class*="SideNav"], [class*="Avatar"], [data-e2e*="avatar"], .ttnow-media-host, [data-ttnow-stage-host="1"]')) continue;
          if (!im.closest('main, [role="dialog"], [class*="DivBrowserModeContainer"]')) continue;
          var iStage = findImageStageHost(im);
          if (iStage) mountStageDownloadButton(iStage, im);
        }
        var existingBtns = document.querySelectorAll('.ttnow-download-btn');
        for (var k = 0; k < existingBtns.length; k++) {
          var btn = existingBtns[k];
          var st = btn.parentElement;
          if (!st || !st.isConnected || !st.querySelector('video, img')) {
            btn.remove();
            if (st && st.getAttribute('data-ttnow-stage-host') === '1') {
              st.removeAttribute('data-ttnow-stage-host');
            }
          }
        }
      }

      // ── Downloader Extraction & Bridge ───────────────────────────────────
      function videoIdFromUrl(url) {
        var m = String(url || '').match(/\/(?:video|photo)\/(\d{15,22})/i);
        return m ? m[1] : '';
      }

      function normalizeTikTokPostUrl(url) {
        var raw = String(url || '').trim();
        if (!raw) return '';
        try {
          var u = new URL(raw, window.location.href);
          if (u.hostname === 'tiktok.com' || u.hostname.endsWith('.tiktok.com')) {
            return u.origin + u.pathname;
          }
        } catch (_) {}
        return '';
      }

      function findPostContainerForMedia(mediaEl) {
        if (!mediaEl || !mediaEl.closest) return null;
        return mediaEl.closest('[data-e2e="recommend-list-item-container"], [data-e2e="browse-video"], [data-e2e="user-post-item"], [class*="DivItemContainer"], [class*="DivBrowserModeContainer"], article') || mediaEl.parentElement;
      }

      function postUrlForMedia(mediaEl) {
        var container = findPostContainerForMedia(mediaEl);
        if (container) {
          var link = container.querySelector('a[href*="/video/"], a[href*="/photo/"]');
          if (link && link.href) {
            var u = normalizeTikTokPostUrl(link.href);
            if (videoIdFromUrl(u)) return u;
          }
        }
        var cur = normalizeTikTokPostUrl(window.location.href);
        return videoIdFromUrl(cur) ? cur : '';
      }

      function idFromXgAttribute(val) {
        var s = String(val || '').trim();
        if (!s) return '';
        var m = s.match(/(?:^|-)(\d{15,22})$/);
        return m ? m[1] : '';
      }

      function extractVideoIdFromStageDom(stage, mediaEl, container) {
        var scopes = [stage, mediaEl && mediaEl.parentElement, container];
        for (var s = 0; s < scopes.length; s++) {
          var sc = scopes[s];
          if (!(sc instanceof Element)) continue;
          var link = sc.querySelector('a[href*="/video/"], a[href*="/photo/"]');
          if (link && link.href) {
            var lid = videoIdFromUrl(link.href);
            if (lid) return lid;
          }
          var ownId = idFromXgAttribute(sc.id) || idFromXgAttribute(sc.getAttribute('data-xgplayerid'));
          if (ownId) return ownId;
          var xg = sc.querySelector('[id^="xgwrapper-"], [data-xgplayerid]');
          if (xg) {
            var xid = idFromXgAttribute(xg.id) || idFromXgAttribute(xg.getAttribute('data-xgplayerid'));
            if (xid) return xid;
          }
        }
        for (var el = mediaEl || stage; el instanceof Element; el = el.parentElement) {
          var aid = idFromXgAttribute(el.id) || idFromXgAttribute(el.getAttribute('data-xgplayerid'));
          if (aid) return aid;
          if (el === container) break;
        }
        return '';
      }

      function extractFromRehydration(targetId) {
        try {
          var u = window.__UNIVERSAL_DATA_FOR_REHYDRATION__;
          var scope = u && u.__DEFAULT_SCOPE__;
          if (scope) {
            var detail = scope['webapp.video-detail'] && scope['webapp.video-detail'].itemInfo && scope['webapp.video-detail'].itemInfo.itemStruct;
            if (detail && (!targetId || String(detail.id) === targetId)) {
              return JSON.stringify(detail);
            }
            var scopeKeys = Object.keys(scope);
            for (var i = 0; i < scopeKeys.length; i++) {
              var sec = scope[scopeKeys[i]];
              if (sec && Array.isArray(sec.itemList)) {
                for (var j = 0; j < sec.itemList.length; j++) {
                  var item = sec.itemList[j];
                  if (item && (!targetId || String(item.id || item.aweme_id || '') === targetId)) {
                    return JSON.stringify(item);
                  }
                }
              }
            }
          }
          if (window.SIGI_STATE && window.SIGI_STATE.ItemModule) {
            if (targetId && window.SIGI_STATE.ItemModule[targetId]) {
              return JSON.stringify(window.SIGI_STATE.ItemModule[targetId]);
            }
          }
        } catch (_) {}
        return '';
      }

      function findReactKeyOnElement(el) {
        var keys;
        try { keys = Object.keys(el); } catch (_) { return ''; }
        var propsKey = '';
        for (var i = 0; i < keys.length; i++) {
          if (keys[i].indexOf('__reactFiber$') === 0) return keys[i];
          if (!propsKey && keys[i].indexOf('__reactProps$') === 0) propsKey = keys[i];
        }
        return propsKey;
      }

      function extractVideoIdFromReactFiber(anchorEl) {
        if (!(anchorEl instanceof Element)) return '';
        try {
          var pickId = function (obj) {
            if (!obj || typeof obj !== 'object') return '';
            var candidates = [
              obj.id, obj.itemId, obj.awemeId, obj.aweme_id, obj.videoId,
              obj.videoSource && obj.videoSource.id,
              obj.itemInfo && obj.itemInfo.itemStruct && (obj.itemInfo.itemStruct.id || obj.itemInfo.itemStruct.aweme_id),
              obj.itemStruct && (obj.itemStruct.id || obj.itemStruct.aweme_id),
              obj.item && (obj.item.id || obj.item.aweme_id),
              obj.data && (obj.data.id || obj.data.aweme_id)
            ];
            for (var c = 0; c < candidates.length; c++) {
              var v = String(candidates[c] || '');
              if (/^\d{15,22}$/.test(v)) return v;
            }
            return '';
          };
          for (var el = anchorEl, hop = 0; el && hop < 12; el = el.parentElement, hop++) {
            var fKey = findReactKeyOnElement(el);
            if (!fKey) continue;
            var fib = el[fKey];
            if (fKey.indexOf('__reactProps$') === 0) {
              var direct = pickId(fib);
              if (direct) return direct;
              continue;
            }
            for (var depth = 0; fib && depth < 35; depth++, fib = fib.return) {
              var hit = pickId(fib.memoizedProps) || pickId(fib.pendingProps);
              if (hit) return hit;
            }
          }
        } catch (_) {}
        return '';
      }

      function extractFromReactFiber(anchorEl, targetId) {
        if (!(anchorEl instanceof Element)) return '';
        try {
          var seen = new Set();
          var looksLikeAweme = function (obj) {
            if (!obj || typeof obj !== 'object') return false;
            var id = String(obj.id || obj.aweme_id || '');
            if (!/^\d{15,22}$/.test(id)) return false;
            if (targetId && id !== targetId) return false;
            return Boolean(obj.video || obj.imagePost || obj.image_post_info || obj.desc || obj.author);
          };
          var bfs = function (root) {
            var q = [{ v: root, d: 0 }];
            var count = 0;
            while (q.length && count < 100) {
              var cur = q.shift();
              var v = cur.v;
              if (!v || typeof v !== 'object' || v === window || seen.has(v) || cur.d > 6) continue;
              seen.add(v);
              count++;
              if (looksLikeAweme(v)) return v;
              var keys;
              try { keys = Object.keys(v); } catch (_) { continue; }
              for (var i = 0; i < Math.min(keys.length, 32); i++) {
                var k = keys[i];
                if (k === 'return' || k === 'child' || k === 'sibling' || k === '_owner' || k === 'stateNode') continue;
                try { q.push({ v: v[k], d: cur.d + 1 }); } catch (_) {}
              }
            }
            return null;
          };
          for (var el = anchorEl, hop = 0; el && hop < 12; el = el.parentElement, hop++) {
            var fKey = findReactKeyOnElement(el);
            if (!fKey) continue;
            var fib = el[fKey];
            if (fKey.indexOf('__reactProps$') === 0) {
              var pHit = bfs(fib);
              if (pHit) return JSON.stringify(pHit);
              continue;
            }
            for (var depth = 0; fib && depth < 35; depth++, fib = fib.return) {
              var hit = bfs(fib.memoizedProps || fib.pendingProps || fib);
              if (hit) return JSON.stringify(hit);
            }
          }
        } catch (_) {}
        return '';
      }

      function extractAuthorUsernameFromContainer(container) {
        if (!(container instanceof Element)) return '';
        var authorEl = container.querySelector('[data-e2e="video-author-uniqueid"], [data-e2e="browse-username"], a[href^="/@"]');
        if (!authorEl) return '';
        var href = authorEl.getAttribute && authorEl.getAttribute('href');
        if (href) {
          var hm = String(href).match(/^\/@([^/?#]+)/);
          if (hm && hm[1] && hm[1] !== 'i') return hm[1].trim();
        }
        var raw = String(authorEl.textContent || '').replace(/^@/, '').trim();
        return raw && raw !== 'i' ? raw : '';
      }

      function extractSeedFromDom(container, mediaEl, postUrl, videoIdOverride) {
        var scope = container || (mediaEl && mediaEl.parentElement) || document;
        var items = [];
        var seen = new Set();
        var vids = scope.querySelectorAll('video');
        for (var i = 0; i < vids.length; i++) {
          var v = vids[i];
          var src = v.currentSrc || v.src || (v.querySelector('source') && v.querySelector('source').src) || '';
          if (src && /^https?:\/\//i.test(src) && !seen.has(src)) {
            seen.add(src);
            items.push({ kind: 'video', thumbnailUrl: v.poster || '', urls: [src] });
          }
        }
        if (!items.length) {
          var imgs = scope.querySelectorAll('img');
          for (var j = 0; j < imgs.length; j++) {
            var im = imgs[j];
            if (!isPostImage(im)) continue;
            var isrc = im.currentSrc || im.src || '';
            if (isrc && /^https?:\/\//i.test(isrc) && !seen.has(isrc)) {
              seen.add(isrc);
              items.push({ kind: 'image', thumbnailUrl: isrc, urls: [isrc] });
            }
          }
        }
        var captionEl = scope.querySelector('[data-e2e="video-desc"], [data-e2e="browse-video-desc"]');
        var username = extractAuthorUsernameFromContainer(scope);
        var id = videoIdOverride || videoIdFromUrl(postUrl);
        if (!items.length && !id) return '';
        try {
          return JSON.stringify({
            ttnowSeed: true,
            videoId: id,
            username: username,
            postUrl: postUrl || '',
            caption: captionEl ? (captionEl.textContent || '').trim() : '',
            items: items
          });
        } catch (_) {
          return '';
        }
      }

      async function openNativeDownloader(initialUrl, targetMedia, stage) {
        var container = findPostContainerForMedia(targetMedia || stage);
        var initNorm = normalizeTikTokPostUrl(initialUrl);
        var detectedUrl = (videoIdFromUrl(initNorm) ? initNorm : '')
          || (targetMedia ? postUrlForMedia(targetMedia) : '')
          || (stage ? postUrlForMedia(stage) : '')
          || '';
        var id = videoIdFromUrl(detectedUrl)
          || extractVideoIdFromStageDom(stage, targetMedia, container)
          || extractVideoIdFromReactFiber(targetMedia || stage || container)
          || videoIdFromUrl(window.location.href)
          || '';

        var payloadJson = getCachedTikTokItem(id)
          || extractFromRehydration(id)
          || extractFromReactFiber(targetMedia || stage || container, id)
          || '';

        var parsedUser = '';
        var parsedIsPhoto = false;
        if (payloadJson) {
          try {
            var parsed = JSON.parse(payloadJson);
            if (parsed && typeof parsed === 'object') {
              if (!id) {
                var pid = String(parsed.id || parsed.aweme_id || parsed.videoId || '');
                if (/^\d{15,22}$/.test(pid)) id = pid;
              }
              parsedUser = String(
                (parsed.author && parsed.author.uniqueId) ||
                parsed.authorUniqueId ||
                parsed.username ||
                ''
              ).replace(/^@/, '').trim();
              parsedIsPhoto = Boolean(parsed.imagePost || parsed.image_post_info);
            }
          } catch (_) {}
        }

        if (!parsedUser) {
          parsedUser = extractAuthorUsernameFromContainer(container);
        }
        if (id && !videoIdFromUrl(detectedUrl)) {
          detectedUrl = 'https://www.tiktok.com/@' + (parsedUser || 'i') + '/' + (parsedIsPhoto ? 'photo' : 'video') + '/' + id;
        }
        if (!payloadJson) {
          payloadJson = extractSeedFromDom(container, targetMedia, detectedUrl, id) || '';
        }

        var itemIndex = null;
        var itemHint = '';
        if (targetMedia instanceof HTMLImageElement) {
          itemHint = targetMedia.currentSrc || targetMedia.src || '';
        } else if (targetMedia instanceof HTMLVideoElement) {
          itemHint = targetMedia.poster || '';
        }

        if (!getInvoke()) {
          showToast('TikTok-Now Downloader bridge unavailable');
          return false;
        }
        try {
          await nativeInvoke('open_downloader_window', {
            url: detectedUrl || null,
            payloadJson: payloadJson || null,
            itemIndex: itemIndex,
            itemHint: itemHint || null,
            theme: 'dark'
          });
          return true;
        } catch (err) {
          var reason = (err && (err.message || (typeof err === 'string' ? err : String(err)))) || '';
          try { console.error('[TikTok-Now] open_downloader_window failed:', err); } catch (_) {}
          showToast(reason
            ? ('Could not open TikTok-Now Downloader: ' + reason)
            : 'Could not open TikTok-Now Downloader');
          return false;
        }
      }

      async function probePostForDownloader(requestId, targetUrl) {
        if (!requestId || !getInvoke()) return false;
        var id = videoIdFromUrl(targetUrl);
        var payloadJson = getCachedTikTokItem(id) || extractFromRehydration(id) || '';
        if (!payloadJson && id && typeof window.fetch === 'function') {
          try {
            var apiRes = await window.fetch('https://www.tiktok.com/api/item/detail/?itemId=' + encodeURIComponent(id), {
              credentials: 'include'
            });
            if (apiRes.ok) {
              var txt = await apiRes.text();
              ingestTikTokJsonText(txt);
              payloadJson = getCachedTikTokItem(id) || txt;
            }
          } catch (_) {}
        }
        if (!payloadJson && /^https?:\/\//i.test(targetUrl || '') && typeof window.fetch === 'function') {
          try {
            var pageRes = await window.fetch(targetUrl, { credentials: 'include' });
            if (pageRes.ok) payloadJson = await pageRes.text();
          } catch (_) {}
        }
        try {
          await nativeInvoke('submit_downloader_probe', {
            requestId: requestId,
            payloadJson: payloadJson || ''
          });
          return true;
        } catch (_) {
          return false;
        }
      }

      window.probePostForDownloader = probePostForDownloader;
      window.__tiktoknowTools = {
        openDownloader: openNativeDownloader,
        probePostForDownloader: probePostForDownloader
      };

      // ── Download Button Pointer Isolation (No preventDefault on pointerdown!) ──
      function downloadButtonFromEvent(target, x, y) {
        if (target && target.closest) {
          var direct = target.closest('.ttnow-download-btn');
          if (direct) return direct;
        }
        if (typeof document.elementsFromPoint === 'function' && Number.isFinite(x) && Number.isFinite(y)) {
          var els = document.elementsFromPoint(x, y);
          for (var i = 0; i < els.length; i++) {
            var b = els[i].closest && els[i].closest('.ttnow-download-btn');
            if (b) return b;
          }
        }
        return null;
      }

      var _activeDownloadPress = null;
      var _activeDownloadMouse = false;
      var _swallowDownloadClickAt = 0;

      window.addEventListener('pointerdown', function (e) {
        var btn = downloadButtonFromEvent(e.target, e.clientX, e.clientY);
        if (!btn) return;
        if (e.button !== 0 && e.pointerType === 'mouse') return;
        e.stopPropagation();
        e.stopImmediatePropagation();
        _activeDownloadPress = { btn: btn, pointerId: e.pointerId };
      }, true);

      window.addEventListener('pointerup', function (e) {
        var press = _activeDownloadPress;
        _activeDownloadPress = null;
        if (!press || e.pointerId !== press.pointerId) return;
        if (e.button !== 0 && e.pointerType === 'mouse') return;
        e.stopPropagation();
        e.stopImmediatePropagation();
        var rel = downloadButtonFromEvent(e.target, e.clientX, e.clientY);
        var rect = press.btn.getBoundingClientRect();
        var near = e.clientX >= rect.left - 12 && e.clientX <= rect.right + 12 && e.clientY >= rect.top - 12 && e.clientY <= rect.bottom + 12;
        if (rel === press.btn || (!rel && near)) {
          var meta = _stageTargetMedia.get(press.btn);
          var liveStage = (meta && meta.stage && meta.stage.isConnected) ? meta.stage : press.btn.parentElement;
          var liveMedia = (meta && meta.media && meta.media.isConnected && liveStage && liveStage.contains(meta.media))
            ? meta.media
            : (liveStage ? liveStage.querySelector('video, img') : null);
          openNativeDownloader('', liveMedia, liveStage);
        }
        _swallowDownloadClickAt = Date.now();
      }, true);

      window.addEventListener('mousedown', function (e) {
        var btn = downloadButtonFromEvent(e.target, e.clientX, e.clientY);
        if (!btn) return;
        _activeDownloadMouse = true;
        e.stopPropagation();
        e.stopImmediatePropagation();
      }, true);

      window.addEventListener('mouseup', function (e) {
        var wasMouse = _activeDownloadMouse;
        _activeDownloadMouse = false;
        if (!wasMouse && !downloadButtonFromEvent(e.target, e.clientX, e.clientY)) return;
        e.stopPropagation();
        e.stopImmediatePropagation();
      }, true);

      window.addEventListener('pointercancel', function (e) {
        if (_activeDownloadPress && e.pointerId === _activeDownloadPress.pointerId) {
          _activeDownloadPress = null;
        }
        _activeDownloadMouse = false;
      }, true);

      window.addEventListener('blur', function () {
        _activeDownloadPress = null;
        _activeDownloadMouse = false;
      });

      window.addEventListener('dblclick', function (e) {
        if (downloadButtonFromEvent(e.target, e.clientX, e.clientY)) {
          e.preventDefault();
          e.stopPropagation();
          e.stopImmediatePropagation();
        }
      }, true);

      window.addEventListener('click', function (e) {
        if (_swallowDownloadClickAt && Date.now() - _swallowDownloadClickAt < 400) {
          _swallowDownloadClickAt = 0;
          e.preventDefault();
          e.stopPropagation();
          e.stopImmediatePropagation();
          return;
        }
        var btn = downloadButtonFromEvent(e.target, e.clientX, e.clientY);
        if (!btn) return;
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation();
        var meta = _stageTargetMedia.get(btn);
        openNativeDownloader('', meta ? meta.media : null, meta ? meta.stage : null);
      }, true);

      // ── 5c. Photo-post Drag Navigation (Exempts .ttnow-download-btn!) ─────
      var CAROUSEL_DRAG_THRESHOLD = 60;
      var CAROUSEL_CLAIM_PX = 6;
      var _carouselDrag = null;
      var _suppressClickUntil = 0;
      var _suppressClickWrap = null;

      function findEdgeButtons(wrap, r) {
        var buttons = wrap.querySelectorAll('button');
        var leftBtn = null, rightBtn = null;
        for (var j = 0; j < buttons.length; j++) {
          if (buttons[j].closest('.ttnow-download-btn')) continue;
          var br = buttons[j].getBoundingClientRect();
          if (br.width === 0 || br.height === 0) continue;
          var insideX = br.left >= r.left - 10 && br.right <= r.right + 10;
          var vCentered = Math.abs((br.top + br.height / 2) - (r.top + r.height / 2)) <= r.height / 2;
          if (!insideX || !vCentered) continue;
          if (br.right < r.left + r.width / 2 && br.left < r.left + r.width * 0.35) leftBtn = buttons[j];
          if (br.left > r.left + r.width / 2 && br.right > r.right - r.width * 0.35) rightBtn = buttons[j];
        }
        return (leftBtn && rightBtn) ? { next: rightBtn, prev: leftBtn } : null;
      }

      function findCarouselTrack(wrap) {
        var t = wrap.querySelector('[style*="transform"]');
        if (t && t.querySelectorAll('img').length >= 2) return t;
        var cands = wrap.querySelectorAll('div');
        for (var i = 0; i < cands.length; i++) {
          var c = cands[i];
          if (c.querySelectorAll('img').length >= 2 && (c.style.transform || c.style.transition)) return c;
        }
        return null;
      }

      function findPhotoContext(target) {
        var el = target;
        for (var depth = 0; el && depth < 8; depth++, el = el.parentElement) {
          if (!el.querySelectorAll || el === document.body || el === document.documentElement) continue;
          var next = el.querySelector('[data-e2e="arrow-next"]');
          var prev = el.querySelector('[data-e2e="arrow-prev"]');
          if (next && prev) {
            return { wrap: el, next: next, prev: prev, track: findCarouselTrack(el), single: false };
          }
          if (el.querySelector('video')) return null;
          var imgs = el.querySelectorAll('img');
          var big = 0;
          for (var i = 0; i < imgs.length; i++) {
            if ((imgs[i].naturalWidth || imgs[i].width || 0) >= 200) big++;
          }
          var r = el.getBoundingClientRect();
          if (r.width > 0 && (r.width < 200 || r.height < 150)) continue;
          if (imgs.length >= 2) {
            if (r.width > 0 && big < 2) continue;
            var fb = findEdgeButtons(el, r);
            if (fb) return { wrap: el, next: fb.next, prev: fb.prev, track: findCarouselTrack(el), single: false };
            continue;
          }
          if (r.width > 0 && big < 1) continue;
          if (imgs.length < 1) continue;
          return { wrap: el, next: null, prev: null, track: null, single: true };
        }
        return null;
      }

      document.addEventListener('pointerdown', function (e) {
        if (window.__tiktoknow_is_dm()) return;
        if (e.target && e.target.closest &&
          e.target.closest('.ttnow-download-btn, input, textarea, select, [contenteditable=""], [contenteditable="true"]')) return;
        var ctx = findPhotoContext(e.target);
        if (!ctx) return;
        _carouselDrag = {
          ctx: ctx,
          startX: e.clientX, startY: e.clientY,
          dx: 0, dy: 0, active: false, bailed: false, moved: false,
          savedTrackStyle: ctx.track ? ctx.track.getAttribute('style') : null
        };
      }, true);

      document.addEventListener('pointermove', function (e) {
        if (!_carouselDrag) return;
        var d = _carouselDrag;
        d.dx = e.clientX - d.startX;
        d.dy = e.clientY - d.startY;
        if (Math.abs(d.dx) + Math.abs(d.dy) > 6) d.moved = true;
        if (!d.active) {
          if (Math.abs(d.dy) > 14 && Math.abs(d.dy) > Math.abs(d.dx)) {
            d.bailed = true;
            return;
          }
          if (Math.abs(d.dx) <= CAROUSEL_CLAIM_PX || Math.abs(d.dx) <= Math.abs(d.dy)) return;
          d.active = true;
        }
        e.preventDefault();
        if (d.ctx.track) {
          d.ctx.track.style.transition = 'none';
          d.ctx.track.style.transform = 'translateX(' + d.dx + 'px)';
        }
      }, true);

      function resetCarouselDrag() {
        if (!_carouselDrag) return null;
        var d = _carouselDrag;
        _carouselDrag = null;
        if (d.ctx.track) {
          if (d.savedTrackStyle === null) d.ctx.track.removeAttribute('style');
          else d.ctx.track.setAttribute('style', d.savedTrackStyle);
        }
        return d;
      }

      document.addEventListener('pointerup', function () {
        var d = resetCarouselDrag();
        if (!d) return;
        if (d.bailed || !d.active || Math.abs(d.dx) < CAROUSEL_DRAG_THRESHOLD) {
          if (d.moved) { _suppressClickUntil = Date.now() + 400; _suppressClickWrap = d.ctx.wrap; }
          return;
        }
        if (d.dx < 0 && d.ctx.next) d.ctx.next.click();
        else if (d.dx > 0 && d.ctx.prev) d.ctx.prev.click();
        if (d.moved) { _suppressClickUntil = Date.now() + 400; _suppressClickWrap = d.ctx.wrap; }
      }, true);

      document.addEventListener('pointercancel', resetCarouselDrag, true);
      window.addEventListener('blur', resetCarouselDrag);

      document.addEventListener('click', function (e) {
        if (e.target && e.target.closest &&
          e.target.closest('.ttnow-download-btn, [data-e2e="arrow-prev"], [data-e2e="arrow-next"]')) return;
        if (Date.now() < _suppressClickUntil && _suppressClickWrap &&
          e.target && _suppressClickWrap.contains(e.target)) {
          e.preventDefault();
          e.stopPropagation();
        }
      }, true);

      document.addEventListener('dragstart', function (e) {
        if (_carouselDrag) e.preventDefault();
      }, true);

      // ── Unified Debounced MutationObserver (Zero full-page innerText) ────
      var _scanTimer = null;
      var _lastScrollScanAt = 0;
      function scheduleStageScan(delayMs) {
        if (_scanTimer !== null) return;
        var base = isLinuxWebKitRuntime() ? 450 : 140;
        var wait = typeof delayMs === 'number' ? delayMs : base;
        if (Date.now() - _lastScrollScanAt < 300) {
          wait = Math.max(wait, isLinuxWebKitRuntime() ? 500 : 250);
        }
        _scanTimer = setTimeout(function () {
          _scanTimer = null;
          _lastScrollScanAt = Date.now();
          scanStageButtons();
        }, wait);
      }

      new MutationObserver(function (mutations) {
        var needsScan = false;
        for (var i = 0; i < mutations.length; i++) {
          var added = mutations[i].addedNodes;
          for (var j = 0; j < added.length; j++) {
            var node = added[j];
            if (node.nodeType !== 1) continue;
            if (node.id === 'tiktok-now-toast' || (node.classList && node.classList.contains('ttnow-download-btn'))) continue;
            if (node.tagName === 'VIDEO') {
              setupVideoListeners(node);
              needsScan = true;
            } else if (node.querySelector) {
              if (node.querySelector('video, img')) needsScan = true;
              if (checkAuthToastNodes(node)) {
                var handle = getTikTokUserHandle();
                var nextTitle = handle ? ('TIKTOKNOW:' + handle) : 'TIKTOKNOW_LOGGED_IN';
                if (document.title !== nextTitle) document.title = nextTitle;
              }
            }
          }
        }
        if (needsScan) scheduleStageScan();
      }).observe(document.documentElement, { childList: true, subtree: true });

      window.addEventListener('scroll', function () {
        scheduleStageScan();
      }, { capture: true, passive: true });
      window.addEventListener('resize', function () {
        scheduleStageScan();
      }, { passive: true });

      if (document.readyState === 'complete') {
        scanStageButtons();
      } else {
        window.addEventListener('load', scanStageButtons, { once: true });
      }

      // ── 5b. Pause-on-Minimize & Resume-on-Restore ────────────────────────
      window.__onWindowHidden = function () {
        var v = activeVideo || document.querySelector('video');
        var wasPlaying = !!(v && !v.paused);
        _playingAudios = Array.from(document.querySelectorAll('audio')).filter(function (a) { return !a.paused; });
        _resumeOnVisible = _intendedPlaying || wasPlaying || _playingAudios.length > 0;
        window.pauseAllVideos();
        if (wasPlaying && v && !window.__tiktoknow_is_dm()) {
          try {
            var r = v.getBoundingClientRect();
            var cx = (r && r.width) ? r.left + r.width / 2 : window.innerWidth / 2;
            var cy = (r && r.height) ? r.top + r.height / 2 : window.innerHeight / 2;
            v.dispatchEvent(new MouseEvent('click', {
              bubbles: true, cancelable: true, clientX: cx, clientY: cy, view: window
            }));
            if (!v.paused) v.pause();
          } catch (_) {}
          _wasMutedBeforeHide = v.muted;
          _mutedByHide = !v.muted;
          if (!v.muted) v.muted = true;
        }
      };

      window.__resumeIfNeeded = function () {
        if (!_resumeOnVisible) return;
        _resumeOnVisible = false;
        var v = activeVideo || document.querySelector('video');
        if (!v) {
          _playingAudios.forEach(function (a) {
            if (a.paused) {
              var p = a.play();
              if (p && p.catch) p.catch(function () {});
            }
          });
          _playingAudios = [];
          return;
        }
        if (_mutedByHide) {
          _programmaticAudio = true;
          try { v.muted = _wasMutedBeforeHide; } finally { _programmaticAudio = false; }
          _mutedByHide = false;
        }
        if (v.paused) {
          var p2 = v.play();
          if (p2 && p2.catch) p2.catch(function () {});
        }
      };

      window.__tiktoknowPauseReport = function () {
        var v = activeVideo || document.querySelector('video');
        var media = document.querySelectorAll('video, audio').length;
        return JSON.stringify({
          media: media,
          active: v ? {
            paused: v.paused,
            muted: v.muted,
            time: Math.round((v.currentTime || 0) * 10) / 10
          } : null,
          intended: _intendedPlaying,
          resumeFlag: _resumeOnVisible,
          isDM: window.__tiktoknow_is_dm()
        });
      };

      document.addEventListener('visibilitychange', function () {
        if (document.hidden) {
          window.__onWindowHidden();
        } else {
          window.__resumeIfNeeded();
          reportThemeBackground(false);
          window.__tiktoknowEnsureForegroundAudio();
        }
        syncPollTick();
      });

      // ── Theme Background Sync (TTNOWBG:r,g,b) ────────────────────────────
      var _lastThemeBgKey = null;
      function reportThemeBackground(force) {
        try {
          if (!isTikTokPage) return;
          if (!force && document.hidden) return;
          var body = document.body;
          if (!body) return;
          var bg = getComputedStyle(body).backgroundColor;
          if (!bg) return;
          var alpha = bg.match(/^rgba\([^)]*,\s*([\d.]+)\)$/);
          if (alpha && parseFloat(alpha[1]) < 1) return;
          var m = bg.match(/rgba?\((\d+)[,\s]+(\d+)[,\s]+(\d+)/);
          if (!m) return;
          var key = m[1] + ',' + m[2] + ',' + m[3];
          if (key === _lastThemeBgKey) return;
          _lastThemeBgKey = key;
          document.title = 'TTNOWBG:' + key;
        } catch (_) {}
      }
      [400, 1500, 4000].forEach(function (ms) {
        setTimeout(function () { reportThemeBackground(false); }, ms);
      });

      // ── Playback Helpers ─────────────────────────────────────────────────
      window.pauseAllVideos = function () {
        document.querySelectorAll('video, audio').forEach(function (m) {
          if (!m.paused) m.pause();
        });
      };

      window.togglePlayPause = function () {
        if (window.__tiktoknow_is_dm()) return;
        var v = activeVideo || document.querySelector('video');
        if (!v) return;
        if (v.paused) {
          var p = v.play();
          if (p && p.catch) p.catch(function () {});
          _intendedPlaying = true;
        } else {
          v.pause();
          _intendedPlaying = false;
        }
      };

      window.toggleMute = function () {
        var v = activeVideo || document.querySelector('video');
        if (v) {
          _userTouchedAudio = true;
          v.dataset.ttnowUserAudio = '1';
          v.muted = !v.muted;
          showToast(v.muted ? 'Muted' : 'Unmuted');
        }
      };

      window.seekBy = function (delta) {
        if (window.__tiktoknow_is_dm()) return;
        var v = activeVideo || document.querySelector('video');
        if (!v) {
          var nextBtn = document.querySelector('[data-e2e="arrow-next"]');
          var prevBtn = document.querySelector('[data-e2e="arrow-prev"]');
          if (delta > 0 && nextBtn) nextBtn.click();
          else if (delta < 0 && prevBtn) prevBtn.click();
          return;
        }
        if (isFinite(v.duration)) {
          v.currentTime = Math.min(Math.max(0, v.currentTime + delta), v.duration);
        }
      };

      window.togglePip = function () {
        var v = activeVideo || document.querySelector('video');
        if (!v) return;
        if (document.pictureInPictureElement) {
          document.exitPictureInPicture().catch(function () {});
        } else if (v.requestPictureInPicture) {
          v.requestPictureInPicture().catch(function () {});
        }
      };

      function triggerPngDownload(dataUrl) {
        var a = document.createElement('a');
        a.href = dataUrl;
        a.download = 'tiktok-now-frame-' + Date.now() + '.png';
        document.body.appendChild(a);
        a.click();
        a.remove();
        showToast('Frame captured');
      }

      window.captureFrame = function () {
        var v = activeVideo || document.querySelector('video');
        if (!v || !v.videoWidth || !v.videoHeight) {
          showToast('No video frame available');
          return;
        }
        try {
          var c = document.createElement('canvas');
          c.width = v.videoWidth;
          c.height = v.videoHeight;
          c.getContext('2d').drawImage(v, 0, 0);
          triggerPngDownload(c.toDataURL('image/png'));
        } catch (_) {
          // CORS-tainted video element fallback: retry via temporary CORS-enabled clone at currentTime
          var src = v.currentSrc || v.src;
          if (!src || src.indexOf('blob:') === 0) {
            showToast('Frame capture blocked by TikTok CORS');
            return;
          }
          try {
            var tmp = document.createElement('video');
            tmp.crossOrigin = 'anonymous';
            tmp.muted = true;
            tmp.preload = 'auto';
            var done = false;
            var cleanup = function () {
              tmp.removeAttribute('src');
              tmp.load();
            };
            var timer = setTimeout(function () {
              if (!done) {
                done = true;
                cleanup();
                showToast('Frame capture blocked by TikTok CORS');
              }
            }, 3500);
            tmp.addEventListener('seeked', function () {
              if (done) return;
              done = true;
              clearTimeout(timer);
              try {
                var c2 = document.createElement('canvas');
                c2.width = tmp.videoWidth || v.videoWidth;
                c2.height = tmp.videoHeight || v.videoHeight;
                c2.getContext('2d').drawImage(tmp, 0, 0);
                triggerPngDownload(c2.toDataURL('image/png'));
              } catch (_) {
                showToast('Frame capture blocked by TikTok CORS');
              } finally {
                cleanup();
              }
            }, { once: true });
            tmp.addEventListener('loadeddata', function () {
              try { tmp.currentTime = v.currentTime || 0; } catch (_) {}
            }, { once: true });
            tmp.addEventListener('error', function () {
              if (!done) {
                done = true;
                clearTimeout(timer);
                cleanup();
                showToast('Frame capture blocked by TikTok CORS');
              }
            }, { once: true });
            tmp.src = src;
          } catch (_) {
            showToast('Frame capture blocked by TikTok CORS');
          }
        }
      };

      window.copyCurrentUrl = function () {
        var url = window.location.href;
        function fallbackCopy(onDone) {
          var ta = document.createElement('textarea');
          ta.value = url;
          ta.style.position = 'fixed';
          ta.style.opacity = '0';
          document.body.appendChild(ta);
          ta.select();
          var ok = false;
          try { ok = document.execCommand('copy'); } catch (_) {}
          ta.remove();
          if (onDone) onDone(ok);
        }
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(url).then(function () {
            showToast('URL copied');
          }, function () {
            fallbackCopy(function (ok) {
              showToast(ok ? 'URL copied' : 'Copy blocked — select the address bar instead');
            });
          });
        } else {
          fallbackCopy(function (ok) {
            showToast(ok ? 'URL copied' : 'Copy blocked — select the address bar instead');
          });
        }
      };

      // ── 6. Native Desktop Navigation Engine ──────────────────────────────
      window.addEventListener('keydown', function (e) {
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (window.__tiktoknow_typing) return;

        var ae = document.activeElement;
        var activeTag = ae ? ae.tagName : '';
        if (activeTag === 'INPUT' || activeTag === 'TEXTAREA' || activeTag === 'BUTTON' || activeTag === 'SELECT'
          || (ae && (ae.isContentEditable || (ae.getAttribute && ae.getAttribute('contenteditable') === 'true')))) {
          if (e.key === 'Escape') { try { ae.blur(); } catch (_) {} }
          return;
        }

        var key = e.key.toLowerCase();
        if (e.key === 'ArrowDown' || key === 'j') {
          window.scrollFeedBy(window.innerHeight * 0.85);
        } else if (e.key === 'ArrowUp' || key === 'k') {
          window.scrollFeedBy(-window.innerHeight * 0.85);
        } else if (e.code === 'Space') {
          e.preventDefault();
          window.togglePlayPause();
        } else if (e.key === 'ArrowLeft') {
          window.seekBy(-5);
        } else if (e.key === 'ArrowRight') {
          window.seekBy(5);
        } else if (key === 'm') {
          window.toggleMute();
        } else if (key === 'p') {
          window.togglePip();
        } else if (key === 's') {
          window.captureFrame();
        } else if (key === 'a') {
          window.toggleAutoScroll();
        } else if (key === 'r') {
          window.location.reload();
        }
      });

      function isTextField(el) {
        return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable);
      }
      window.addEventListener('keydown', function () {
        if (isTextField(document.activeElement)) {
          window.__tiktoknow_typing = true;
        }
      }, true);
      window.addEventListener('focusout', function (e) {
        if (isTextField(e.target) && !isTextField(document.activeElement)) {
          window.__tiktoknow_typing = false;
        }
      }, true);

      // ── 7. Adaptive Idle-Aware Polling (Handle + Stage + BG Sync) ────────
      var POLL_ACTIVE_MS = 1000;
      var POLL_IDLE_MS = 5000;
      var POLL_DEEP_IDLE_MS = 15000;
      var POLL_HIDDEN_MS = 30000;
      var USER_IDLE_AFTER_MS = 15000;
      var USER_DEEP_IDLE_AFTER_MS = 60000;
      var _lastActivity = Date.now();
      var _pollTimer = null;
      var _pollRunning = false;
      var _lastRoute = window.location.href;
      var _lastReportedHandle = '';

      function pollIntervalMs() {
        if (document.hidden) return POLL_HIDDEN_MS;
        var idleFor = Date.now() - _lastActivity;
        if (idleFor >= USER_DEEP_IDLE_AFTER_MS) return POLL_DEEP_IDLE_MS;
        if (idleFor >= USER_IDLE_AFTER_MS) return POLL_IDLE_MS;
        return POLL_ACTIVE_MS;
      }

      function pollTick() {
        _pollTimer = null;
        if (_pollRunning) return;
        _pollRunning = true;
        try {
          var handle = getTikTokUserHandle();
          if (handle && handle !== _lastReportedHandle) {
            _lastReportedHandle = handle;
            document.title = 'TIKTOKNOW:' + handle;
          }
          var curRoute = window.location.href;
          if (curRoute !== _lastRoute) {
            _lastRoute = curRoute;
            scheduleStageScan(100);
          } else if (!document.hidden) {
            scanStageButtons();
          }
          maybeStartupUnmute(activeVideo || document.querySelector('video'));
        } catch (_) {
        } finally {
          _pollRunning = false;
        }
        armPollTick();
      }

      function armPollTick() {
        if (_pollTimer !== null) return;
        _pollTimer = setTimeout(pollTick, pollIntervalMs());
      }

      function syncPollTick() {
        if (_pollTimer !== null) {
          clearTimeout(_pollTimer);
          _pollTimer = null;
        }
        armPollTick();
      }

      function markUserActive() {
        _lastActivity = Date.now();
        syncPollTick();
      }

      window.addEventListener('pointerdown', markUserActive, { capture: true, passive: true });
      window.addEventListener('keydown', markUserActive, { capture: true, passive: true });
      window.addEventListener('wheel', markUserActive, { capture: true, passive: true });
      armPollTick();

      // ── 8. Global External Link Opener ───────────────────────────────────
      window.__tiktoknow_open = function (url) {
        var fn = getInvoke();
        if (fn) {
          fn('open_external_url', { url: url }).catch(function () {
            fn('jump_to_external', { url: url }).catch(function () {
              window.open(url, '_blank');
            });
          });
        } else {
          window.open(url, '_blank');
        }
      };

      // ── 9. Link-Aware External Handler ───────────────────────────────────
      document.addEventListener('click', function (e) {
        var a = e.target && e.target.closest ? e.target.closest('a') : null;
        if (!a) return;
        var href = a.getAttribute('href') || a.href || '';
        if (!href || href.indexOf('#') === 0 || href.indexOf('javascript:') === 0) return;

        var isExternal = false;
        try {
          var u = new URL(href, window.location.href);
          var host = u.hostname.toLowerCase();
          var isTikTok = host === 'tiktok.com' || host.endsWith('.tiktok.com');
          var isLocal = host === 'localhost' || host === '127.0.0.1' || host === '[::1]'
            || host === 'tauri.localhost' || host === 'ipc.localhost' || host.endsWith('.localhost')
            || u.protocol === 'file:' || u.protocol === 'tauri:';
          if (!isTikTok && !isLocal && (u.protocol === 'http:' || u.protocol === 'https:')) {
            isExternal = true;
          }
        } catch (_) {}

        if (isExternal) {
          e.preventDefault();
          e.stopImmediatePropagation();
          window.__tiktoknow_open(a.href || href);
        }
      }, true);

    } catch (e) {
      try { console.error('[TikTok-Now] init failed:', e); } catch (_) {}
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();
