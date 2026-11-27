// Copyright (c) 2026 Benedictus Reynaldo Hartanto (@benedictusrey). All rights reserved.
// TikTok-Now — High-performance desktop client for TikTok (https://github.com/benedictusrey/TikTok-Now)
// Licensed under the MIT License.

(() => {
  const urlInput = document.getElementById("url-input");
  const clearUrlBtn = document.getElementById("clear-url-btn");
  const urlHint = document.getElementById("url-hint");
  const pasteBtn = document.getElementById("paste-btn");
  const inspectBtn = document.getElementById("inspect-btn");
  const openFolderBtn = document.getElementById("open-folder-btn");
  const slideStrip = document.getElementById("slide-strip");
  const slideSelect = document.getElementById("slide-select");
  const prevSlideBtn = document.getElementById("prev-slide-btn");
  const nextSlideBtn = document.getElementById("next-slide-btn");
  const formatSelect = document.getElementById("format-select");
  const captionInput = document.getElementById("caption-input");
  const captionCount = document.getElementById("caption-count");
  const copyCaptionBtn = document.getElementById("copy-caption-btn");
  const saveCaptionBtn = document.getElementById("save-caption-btn");
  const downloadSelectedBtn = document.getElementById("download-selected-btn");
  const downloadSelectedLabel = document.getElementById("download-selected-label");
  const downloadAudioBtn = document.getElementById("download-audio-btn");
  const downloadBundleBtn = document.getElementById("download-bundle-btn");
  const downloadBundleLabel = document.getElementById("download-bundle-label");
  const actionsGrid = document.querySelector(".actions-grid");
  const actionsNote = document.getElementById("actions-note");
  const postSummary = document.getElementById("post-summary");
  const engineBadge = document.getElementById("engine-badge");
  const postSection = document.getElementById("post-section");
  const statusFooter = document.getElementById("status-footer");
  const statusBox = document.getElementById("status-box");
  const statusOpenBtn = document.getElementById("status-open-btn");
  const progressTrack = document.getElementById("progress-track");
  const progressFill = document.getElementById("progress-fill");
  const previewBox = document.getElementById("preview-box");
  const previewEmpty = document.getElementById("preview-empty");
  const previewEmptyText = document.getElementById("preview-empty-text");
  const previewTag = document.getElementById("preview-tag");
  const previewZoomBar = document.getElementById("preview-zoom-bar");
  const previewZoomSlider = document.getElementById("preview-zoom-slider");
  const previewZoomLabel = document.getElementById("preview-zoom-label");
  const previewZoomOutBtn = document.getElementById("preview-zoom-out-btn");
  const previewZoomInBtn = document.getElementById("preview-zoom-in-btn");
  const previewZoomResetBtn = document.getElementById("preview-zoom-reset-btn");
  const outputNamePreview = document.getElementById("output-name-preview");
  const copyStemBtn = document.getElementById("copy-stem-btn");
  const audioSpecPreview = document.getElementById("audio-spec-preview");

  const isMac = /mac/i.test(
    navigator.userAgentData?.platform || navigator.platform || navigator.userAgent || ""
  );
  const linkShortcutLabel = isMac ? "Cmd+L" : "Ctrl+L";
  const pasteShortcutLabel = isMac ? "Cmd+V" : "Ctrl+V";

  let currentPost = null;
  let busy = false;
  let activeDownloadMode = "";
  let previewThumb = null;
  let previewZoom = 1;
  let previewPanX = 0;
  let previewPanY = 0;
  let lastAutoClipboardUrl = "";
  let queuedContext = null;

  function invoke(command, args = {}) {
    const fn = window.__TAURI__?.core?.invoke;
    if (typeof fn !== "function") {
      return Promise.reject(new Error("TikTok-Now native bridge is unavailable."));
    }
    return fn(command, args);
  }

  function isTikTokPostLink(text) {
    const clean = String(text || "").trim();
    if (/^\d{15,22}$/.test(clean)) return true;
    return /https?:\/\/(?:www\.|m\.|vm\.|vt\.)?tiktok\.com\/(?:@[^/\s]+\/(?:video|photo)\/\d+|t\/[A-Za-z0-9_-]+|[A-Za-z0-9_-]{6,})/i.test(clean);
  }

  function setStatus(message, tone = "", showOpenFolder = false) {
    statusBox.textContent = String(message || "");
    if (tone) statusBox.setAttribute("data-tone", tone);
    else statusBox.removeAttribute("data-tone");
    if (statusOpenBtn) statusOpenBtn.hidden = !showOpenFolder;
  }

  function setProgress(completed, total) {
    if (!progressTrack || !progressFill) return;
    if (!Number.isFinite(completed) || !Number.isFinite(total) || total <= 0) {
      progressTrack.hidden = true;
      progressFill.style.width = "0%";
      return;
    }
    const pct = Math.max(0, Math.min(100, Math.round((completed / total) * 100)));
    progressTrack.hidden = false;
    progressFill.style.width = `${pct}%`;
  }

  function setPreviewEmptyMessage(text, state = "idle") {
    if (previewBox) previewBox.setAttribute("data-state", state);
    if (previewEmptyText) {
      previewEmptyText.textContent = text;
    } else if (previewEmpty) {
      previewEmpty.textContent = text;
    }
  }

  function updateUrlControls() {
    const val = urlInput.value.trim();
    if (clearUrlBtn) clearUrlBtn.hidden = !val;
    if (urlHint) {
      if (!val) {
        urlHint.textContent = `Auto-inspects on open or paste (${linkShortcutLabel})`;
      } else if (isTikTokPostLink(val)) {
        urlHint.textContent = "TikTok post link detected";
      } else {
        urlHint.textContent = "Enter a tiktok.com/@user/video/... or /photo/... URL";
      }
    }
  }

  function updateCaptionCounter() {
    if (!captionCount) return;
    const len = captionInput.value.length;
    captionCount.textContent = `${len.toLocaleString()} char${len === 1 ? "" : "s"}`;
  }

  function formatOutputPattern(post, item, slideIndex) {
    if (!post || !item) return "username-YYYYMMDD-videoId.mp4";
    const user = String(post.username || "tiktok")
      .trim()
      .replace(/[^A-Za-z0-9._]+/g, "_")
      .replace(/^[._]+|[._]+$/g, "")
      .slice(0, 36) || "tiktok";
    const isoDigits = String(post.createdAt || "").slice(0, 10).replace(/\D+/g, "");
    const datePart = isoDigits.length === 8
      ? isoDigits
      : new Date().toISOString().slice(0, 10).replace(/\D+/g, "");
    const idPart = post.videoId || "post";
    const fmt = selectedFormatForItem(item);
    const ext = fmt?.ext || (item.kind === "image" ? "jpg" : item.kind === "gif" ? "gif" : "mp4");
    const total = post.items?.length || 0;
    const slideNum = total >= 10 ? String(slideIndex + 1).padStart(2, "0") : String(slideIndex + 1);
    const suffix = total > 1 ? `-slide-${slideNum}` : "";
    return `${user}-${datePart}-${idPart}${suffix}.${ext}`;
  }

  function currentSlideIndex() {
    return Math.max(0, Number(slideSelect.value) || 0);
  }

  function currentSlideItem() {
    const idx = currentSlideIndex();
    return currentPost?.items?.[idx] || currentPost?.items?.[0] || null;
  }

  function selectedFormatForItem(item) {
    if (!item || !Array.isArray(item.formats) || !item.formats.length) return null;
    return item.formats.find(f => f.id === formatSelect.value) || item.formats[0] || null;
  }

  function updateAudioAndFormatSpec(item) {
    if (!audioSpecPreview || !item) return;
    const fmt = selectedFormatForItem(item);
    const dims = fmt?.width && fmt?.height ? `${fmt.width}\u00d7${fmt.height}` : "";
    if (fmt?.ext === "gif" || item.kind === "gif") {
      const desc = fmt?.ext === "gif" ? "Animated GIF (FFmpeg)" : "MP4 loop";
      audioSpecPreview.textContent = dims ? `${dims} (${desc})` : desc;
    } else if (item.kind === "video") {
      const codec = fmt?.codecs ? fmt.codecs.toUpperCase() : "";
      const bitrate = fmt?.bandwidth >= 1000 ? `${Math.round(fmt.bandwidth / 1000)} kbps` : "";
      const meta = [codec, bitrate].filter(Boolean).join(" \u00b7 ")
        || (currentPost?.ffmpegAvailable ? "AAC 192 kbps \u00b7 MP4 / M4A" : "MP4 stream");
      audioSpecPreview.textContent = dims ? `${dims} (${meta})` : meta;
    } else {
      audioSpecPreview.textContent = dims ? `${dims} Still image` : "Original resolution image";
    }
    if (outputNamePreview) {
      outputNamePreview.textContent = formatOutputPattern(currentPost, item, currentSlideIndex());
    }
  }

  function setBusy(nextBusy, labelMode = "") {
    busy = nextBusy;
    if (statusFooter) statusFooter.setAttribute("data-busy", nextBusy ? "1" : "0");
    if (postSection) postSection.setAttribute("aria-busy", nextBusy ? "true" : "false");
    if (!nextBusy) setProgress(0, 0);
    inspectBtn.disabled = nextBusy;
    inspectBtn.textContent = nextBusy && labelMode === "inspect" ? "Inspecting..." : "Inspect Link";
    updateActionButtons();
  }

  async function drainQueuedContext() {
    if (!queuedContext || busy) return;
    const next = queuedContext;
    queuedContext = null;
    urlInput.value = next.url || "";
    updateUrlControls();
    await inspectUrl(next.url || "", next.payloadJson || null, next);
  }

  function setBundleButtonLabel(text) {
    if (downloadBundleLabel) {
      downloadBundleLabel.textContent = text;
    } else if (downloadBundleBtn) {
      downloadBundleBtn.textContent = text;
    }
  }

  function updateActionButtons() {
    const items = currentPost?.items || [];
    const hasPost = Boolean(currentPost && Array.isArray(items) && items.length);
    const slideIdx = currentSlideIndex();
    const activeItem = currentSlideItem();
    const hasAnyVideo = hasPost && items.some(item => item.kind === "video");
    const isVideoSlide = Boolean(hasPost && activeItem?.kind === "video");
    const isImageOnlyPost = Boolean(hasPost && !items.some(item => item.kind === "video" || item.kind === "gif"));

    slideSelect.disabled = busy || !hasPost;
    formatSelect.disabled = busy || !hasPost;
    if (prevSlideBtn) prevSlideBtn.disabled = busy || !hasPost || items.length <= 1 || slideIdx <= 0;
    if (nextSlideBtn) nextSlideBtn.disabled = busy || !hasPost || items.length <= 1 || slideIdx >= items.length - 1;
    downloadSelectedBtn.disabled = busy || !hasPost;
    downloadBundleBtn.disabled = busy || !hasPost;
    if (actionsGrid) {
      if (!hasAnyVideo) actionsGrid.setAttribute("data-image-only", "1");
      else actionsGrid.removeAttribute("data-image-only");
    }
    if (downloadAudioBtn) {
      downloadAudioBtn.hidden = !hasAnyVideo;
      downloadAudioBtn.disabled = busy || !isVideoSlide || !currentPost?.ffmpegAvailable;
      downloadAudioBtn.title = !currentPost?.ffmpegAvailable
        ? "Install FFmpeg to extract .m4a audio"
        : isVideoSlide
          ? "Extract 192 kbps stereo AAC (.m4a)"
          : "Audio extraction is available for video items";
    }
    if (isImageOnlyPost) {
      if (items.length > 1) {
        setBundleButtonLabel(`Save All ${items.length} Images + Caption`);
        if (actionsNote) {
          actionsNote.textContent = `Save All ${items.length} Images exports every slide in order alongside caption.txt and metadata.json into one folder.`;
        }
      } else {
        setBundleButtonLabel("Save Image + Caption");
        if (actionsNote) {
          actionsNote.textContent = "Save Image + Caption exports the full-resolution image alongside caption.txt and metadata.json.";
        }
      }
    } else if (items.length > 1) {
      setBundleButtonLabel(`Save Full Post (${items.length} items)`);
      if (actionsNote) {
        actionsNote.textContent = `Save Full Post exports all ${items.length} items alongside caption.txt and metadata.json into one folder.`;
      }
    } else {
      setBundleButtonLabel("Save Full Post");
      if (actionsNote) {
        actionsNote.textContent = "Save Full Post exports the media alongside caption.txt and metadata.json into one folder.";
      }
    }
    if (slideStrip) {
      slideStrip.querySelectorAll("button[data-slide-idx]").forEach(btn => {
        btn.disabled = busy;
      });
    }
    if (copyStemBtn) copyStemBtn.disabled = busy || !hasPost;
    const hasCaptionOrPost = Boolean(currentPost || captionInput.value.trim());
    copyCaptionBtn.disabled = busy || !captionInput.value.trim();
    saveCaptionBtn.disabled = busy || !hasCaptionOrPost;
    updateCaptionCounter();
  }

  function applyPreviewZoomTransform() {
    if (previewThumb) {
      previewThumb.style.transform = previewZoom > 1.001
        ? `translate(${previewPanX}px, ${previewPanY}px) scale(${previewZoom})`
        : "";
    }
    const pct = Math.round(previewZoom * 100);
    if (previewZoomSlider) previewZoomSlider.value = String(pct);
    if (previewZoomLabel) previewZoomLabel.textContent = `${pct}%`;
    if (previewZoomResetBtn) previewZoomResetBtn.disabled = pct <= 100;
    if (previewBox) {
      previewBox.classList.toggle("is-zoomable", Boolean(previewThumb) && pct > 100);
    }
  }

  function resetPreviewZoom() {
    previewZoom = 1;
    previewPanX = 0;
    previewPanY = 0;
    applyPreviewZoomTransform();
  }

  function setPreviewZoom(nextZoom) {
    if (!previewThumb || currentSlideItem()?.kind !== "image") return;
    const clamped = Math.max(1, Math.min(4, Math.round(nextZoom * 100) / 100));
    previewZoom = clamped;
    if (clamped <= 1.001) {
      previewPanX = 0;
      previewPanY = 0;
    }
    applyPreviewZoomTransform();
  }

  function syncPreviewZoomBar(showZoom) {
    if (!previewZoomBar) return;
    previewZoomBar.hidden = !showZoom;
    if (!showZoom) {
      previewBox?.classList.remove("is-panning", "is-zoomable");
      resetPreviewZoom();
    }
  }

  function removePreviewThumb() {
    if (previewThumb) {
      previewThumb.remove();
      previewThumb = null;
    }
    syncPreviewZoomBar(false);
  }

  function updatePreview(item, slideIndex) {
    if (!previewBox || !previewEmpty || !previewTag) return;
    resetPreviewZoom();
    if (!item) {
      removePreviewThumb();
      previewEmpty.hidden = false;
      previewTag.hidden = true;
      setPreviewEmptyMessage("Preview loads automatically on inspection", "idle");
      if (outputNamePreview) outputNamePreview.textContent = "username-YYYYMMDD-videoId.mp4";
      if (audioSpecPreview) audioSpecPreview.textContent = "Original resolution";
      return;
    }

    const fmt = selectedFormatForItem(item);
    const kindTag = fmt?.ext === "gif" || item.kind === "gif"
      ? "GIF"
      : item.kind === "video"
        ? "MP4"
        : (fmt?.ext || "JPG").toUpperCase();
    previewTag.textContent = (currentPost?.items?.length || 0) > 1
      ? `${slideIndex + 1} / ${currentPost.items.length} ${kindTag}`
      : kindTag;
    previewTag.hidden = false;
    const isImageItem = item.kind === "image";

    const thumbCandidates = [
      item.thumbnailUrl,
      ...(item.kind === "image" ? (item.formats || []).map(f => f.videoUrl) : [])
    ].filter((u, idx, arr) => u && /^https?:\/\//i.test(u) && arr.indexOf(u) === idx);
    let thumbIdx = 0;

    if (thumbCandidates.length) {
      if (!previewThumb) {
        previewThumb = document.createElement("img");
        previewThumb.id = "preview-thumb";
        previewThumb.alt = "Selected TikTok media preview";
        previewBox.prepend(previewThumb);
      }
      previewThumb.onload = () => {
        if (previewThumb) previewThumb.hidden = false;
        previewEmpty.hidden = true;
        previewBox.setAttribute("data-state", "ready");
        syncPreviewZoomBar(isImageItem);
      };
      previewThumb.onerror = () => {
        thumbIdx += 1;
        if (thumbIdx < thumbCandidates.length && previewThumb) {
          previewThumb.src = thumbCandidates[thumbIdx];
          return;
        }
        removePreviewThumb();
        previewEmpty.hidden = false;
        setPreviewEmptyMessage("Preview unavailable. Formats are ready to save.", "unavailable");
      };
      previewThumb.src = thumbCandidates[0];
      previewThumb.hidden = false;
      previewEmpty.hidden = true;
      previewBox.setAttribute("data-state", "ready");
      syncPreviewZoomBar(isImageItem);
    } else {
      removePreviewThumb();
      previewEmpty.hidden = false;
      setPreviewEmptyMessage("Preview unavailable. Formats are ready to save.", "unavailable");
    }

    updateAudioAndFormatSpec(item);
  }

  function setPrimaryDownloadLabel(text) {
    if (downloadSelectedLabel) {
      downloadSelectedLabel.textContent = text;
    } else {
      downloadSelectedBtn.textContent = text;
    }
  }

  function syncSlideStrip(slideIndex) {
    if (!slideStrip) return;
    slideStrip.querySelectorAll("button[data-slide-idx]").forEach(btn => {
      const isSelected = Number(btn.getAttribute("data-slide-idx")) === slideIndex;
      btn.setAttribute("aria-pressed", isSelected ? "true" : "false");
      if (isSelected) btn.scrollIntoView?.({ block: "nearest", inline: "center" });
    });
  }

  function renderSlideStrip(items) {
    if (!slideStrip) return;
    slideStrip.replaceChildren();
    if (!Array.isArray(items) || items.length <= 1) {
      slideStrip.hidden = true;
      return;
    }
    slideStrip.hidden = false;
    items.forEach((item, idx) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "slide-strip__btn";
      btn.setAttribute("data-slide-idx", String(idx));
      btn.setAttribute("aria-pressed", "false");
      btn.setAttribute("aria-label", `Select slide ${idx + 1}`);
      btn.title = `Slide ${idx + 1} (${item.kind})`;
      const thumbSrc = item.thumbnailUrl || (item.kind === "image" ? item.formats?.[0]?.videoUrl : "") || "";
      if (/^https?:\/\//i.test(thumbSrc)) {
        const img = document.createElement("img");
        img.className = "slide-strip__thumb";
        img.alt = "";
        img.loading = "lazy";
        img.src = thumbSrc;
        btn.appendChild(img);
      }
      const num = document.createElement("span");
      num.className = "slide-strip__num";
      num.textContent = String(idx + 1);
      btn.appendChild(num);
      if (item.kind === "video" || item.kind === "gif") {
        const badge = document.createElement("span");
        badge.className = "slide-strip__video";
        const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        svg.setAttribute("viewBox", "0 0 24 24");
        svg.setAttribute("width", "9");
        svg.setAttribute("height", "9");
        svg.setAttribute("fill", "currentColor");
        svg.setAttribute("aria-hidden", "true");
        const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
        path.setAttribute("d", "m8 5 11 7-11 7z");
        svg.appendChild(path);
        badge.appendChild(svg);
        btn.appendChild(badge);
      }
      btn.addEventListener("click", () => {
        if (busy) return;
        slideSelect.value = String(idx);
        renderFormatsForSlide(idx);
      });
      slideStrip.appendChild(btn);
    });
  }

  function resolveInitialItemIndex(post, rawUrl, ctx = null) {
    const items = post?.items || [];
    if (items.length <= 1) return 0;
    const hint = String(ctx?.itemHint || "").trim();
    if (hint) {
      const exactIdx = items.findIndex(item =>
        item.thumbnailUrl === hint || (item.formats || []).some(f => f.videoUrl === hint)
      );
      if (exactIdx >= 0) return exactIdx;
    }
    if (Number.isInteger(ctx?.itemIndex) && ctx.itemIndex >= 0 && ctx.itemIndex < items.length) {
      return ctx.itemIndex;
    }
    const idxMatch = String(rawUrl || "").match(/[?&]img_index=(\d+)/i);
    if (idxMatch) {
      const parsed = Number(idxMatch[1]) - 1;
      if (parsed >= 0 && parsed < items.length) return parsed;
    }
    return 0;
  }

  function formatReadyStatus(post, slideIndex) {
    const items = post?.items || [];
    const total = items.length;
    const videoCount = items.filter(i => i.kind === "video" || i.kind === "gif").length;
    const itemNum = slideIndex + 1;
    if (videoCount === 0 && total > 1) {
      return `Ready: ${total} carousel photos found. Viewing slide ${itemNum}.`;
    }
    if (videoCount === 0) {
      return "Ready: full-resolution photo detected.";
    }
    if (total > 1) {
      return `Ready: ${total} media items detected. Viewing item ${itemNum}.`;
    }
    return items[0].kind === "gif"
      ? "Ready: animated GIF detected."
      : "Ready: video stream detected.";
  }

  function updatePrimaryButtonForSelection(item, slideIndex) {
    const totalSlides = currentPost?.items?.length || 0;
    const fmt = selectedFormatForItem(item);
    const ext = fmt?.ext || (item.kind === "image" ? "jpg" : item.kind === "gif" ? "gif" : "mp4");
    if (totalSlides > 1) {
      setPrimaryDownloadLabel(`Download Slide ${slideIndex + 1} (.${ext})`);
    } else if (ext === "gif" || item.kind === "gif") {
      setPrimaryDownloadLabel(`Download GIF (.${ext})`);
    } else if (item.kind === "video") {
      setPrimaryDownloadLabel(`Download Video (.${ext})`);
    } else {
      setPrimaryDownloadLabel(`Download Image (.${ext})`);
    }
  }

  function renderFormatsForSlide(slideIndex) {
    formatSelect.replaceChildren();
    const item = currentPost?.items?.[slideIndex] || currentPost?.items?.[0];
    if (!item || !Array.isArray(item.formats) || !item.formats.length) {
      const opt = document.createElement("option");
      opt.value = "";
      opt.textContent = "No formats detected";
      formatSelect.appendChild(opt);
      setPrimaryDownloadLabel("Download Selected");
      syncSlideStrip(slideIndex);
      updatePreview(item || null, slideIndex);
      updateActionButtons();
      return;
    }
    item.formats.forEach((fmt, idx) => {
      const opt = document.createElement("option");
      opt.value = fmt.id;
      opt.textContent = fmt.label || `${fmt.ext?.toUpperCase() || "MP4"} #${idx + 1}`;
      formatSelect.appendChild(opt);
    });
    updatePrimaryButtonForSelection(item, slideIndex);
    syncSlideStrip(slideIndex);
    updatePreview(item, slideIndex);
    updateActionButtons();
    if (!busy && /^Ready:/i.test(statusBox?.textContent || "")) {
      setStatus(formatReadyStatus(currentPost, slideIndex), "ok");
    }
  }

  function renderMetaChips(post) {
    postSummary.replaceChildren();
    const total = post.items.length;
    const chips = [
      post.username ? `@${post.username}` : "@tiktok",
      post.videoId ? `Post ${post.videoId}` : "TikTok Post",
      `${total} ${total === 1 ? "item" : "items"}`
    ];
    chips.forEach(text => {
      const chip = document.createElement("span");
      chip.className = "meta-chip";
      chip.textContent = text;
      postSummary.appendChild(chip);
    });
  }

  function renderPost(post, rawUrl = "", ctx = null) {
    currentPost = post;
    const currentInput = urlInput.value.trim();
    if (
      post.postUrl &&
      (!currentInput || !isTikTokPostLink(currentInput) || /\/@i\/(?:video|photo)\//i.test(currentInput))
    ) {
      urlInput.value = post.postUrl;
      updateUrlControls();
    }
    renderMetaChips(post);
    engineBadge.textContent = post.ffmpegAvailable
      ? "FFmpeg Audio & GIF Ready"
      : "MP4 Direct";
    engineBadge.setAttribute("data-ready", post.ffmpegAvailable ? "ffmpeg" : "progressive");
    captionInput.value = post.caption || "";

    slideSelect.replaceChildren();
    const total = post.items.length;
    post.items.forEach((item, idx) => {
      const opt = document.createElement("option");
      opt.value = String(idx);
      const kindLabel = item.kind === "gif" ? "GIF" : item.kind === "video" ? "Video" : "Photo";
      opt.textContent = total > 1
        ? `Slide ${idx + 1} / ${total} (${kindLabel})`
        : `Slide 1 (${kindLabel})`;
      slideSelect.appendChild(opt);
    });
    renderSlideStrip(post.items);
    const startIdx = resolveInitialItemIndex(post, rawUrl, ctx);
    slideSelect.value = String(startIdx);
    renderFormatsForSlide(startIdx);
    updateActionButtons();
  }

  async function inspectUrl(rawUrl, seedPayloadJson = null, ctx = null) {
    if (busy) return;
    const targetUrl = String(rawUrl ?? urlInput.value ?? "").trim();
    if (!targetUrl && !seedPayloadJson) {
      setStatus("Paste a TikTok post URL first.", "warn");
      return;
    }
    if (currentPost && targetUrl !== currentPost.postUrl) {
      currentPost = null;
      slideSelect.replaceChildren();
      formatSelect.replaceChildren();
      updateActionButtons();
    }
    setBusy(true, "inspect");
    if (previewBox && !previewThumb && previewBox.getAttribute("data-state") !== "ready") {
      previewEmpty.hidden = false;
      setPreviewEmptyMessage("Inspecting post media and caption...", "loading");
    }
    setStatus("Inspecting TikTok post formats, resolution variants, and caption...");
    try {
      const post = await invoke("inspect_tiktok_post", {
        url: targetUrl,
        seedPayloadJson: seedPayloadJson || null
      });
      renderPost(post, targetUrl, ctx);
      setStatus(formatReadyStatus(post, currentSlideIndex()), "ok");
    } catch (error) {
      if (previewBox && previewBox.getAttribute("data-state") === "loading") {
        setPreviewEmptyMessage("Preview loads automatically on inspection", "idle");
      }
      setStatus(error?.message || String(error), "err");
    } finally {
      setBusy(false);
      await drainQueuedContext();
    }
  }

  async function tryAutoInspectClipboard() {
    if (busy || urlInput.value.trim()) return false;
    try {
      const clip = await navigator.clipboard?.readText?.();
      const clean = String(clip || "").trim();
      if (clean && clean !== lastAutoClipboardUrl && isTikTokPostLink(clean)) {
        lastAutoClipboardUrl = clean;
        urlInput.value = clean;
        updateUrlControls();
        await inspectUrl(clean);
        return true;
      }
    } catch {
      // Clipboard read can require active focus on some platforms.
    }
    return false;
  }

  async function loadPendingContext() {
    try {
      const ctx = await invoke("take_pending_downloader_context");
      if (ctx && (ctx.url || ctx.payloadJson)) {
        if (busy) {
          queuedContext = {
            url: ctx.url || "",
            payloadJson: ctx.payloadJson || null,
            itemIndex: ctx.itemIndex ?? null,
            itemHint: ctx.itemHint || null,
            theme: ctx.theme || null
          };
          return;
        }
        urlInput.value = ctx.url || "";
        updateUrlControls();
        await inspectUrl(ctx.url || "", ctx.payloadJson || null, ctx);
        return;
      }
    } catch {
      // Standalone open without pending context.
    }
    await tryAutoInspectClipboard();
  }

  pasteBtn.addEventListener("click", async () => {
    if (busy) return;
    try {
      const clip = await navigator.clipboard?.readText?.();
      if (clip && clip.trim()) {
        urlInput.value = clip.trim();
        updateUrlControls();
        await inspectUrl(clip.trim());
        return;
      }
    } catch {}
    urlInput.focus();
    setStatus(`Press ${pasteShortcutLabel} to paste link; inspection starts automatically.`);
  });

  if (clearUrlBtn) {
    clearUrlBtn.addEventListener("click", () => {
      if (busy) return;
      urlInput.value = "";
      updateUrlControls();
      urlInput.focus();
    });
  }

  urlInput.addEventListener("input", updateUrlControls);

  urlInput.addEventListener("paste", () => {
    window.setTimeout(() => {
      updateUrlControls();
      const pasted = urlInput.value.trim();
      if (pasted && !busy && isTikTokPostLink(pasted)) {
        inspectUrl(pasted);
      }
    }, 25);
  });

  inspectBtn.addEventListener("click", () => inspectUrl());
  urlInput.addEventListener("keydown", event => {
    if (event.key === "Enter") {
      event.preventDefault();
      inspectUrl();
    }
  });

  if (previewZoomSlider) {
    previewZoomSlider.addEventListener("input", () => setPreviewZoom(Number(previewZoomSlider.value) / 100));
  }
  if (previewZoomOutBtn) {
    previewZoomOutBtn.addEventListener("click", () => setPreviewZoom(previewZoom - 0.15));
  }
  if (previewZoomInBtn) {
    previewZoomInBtn.addEventListener("click", () => setPreviewZoom(previewZoom + 0.15));
  }
  if (previewZoomResetBtn) {
    previewZoomResetBtn.addEventListener("click", resetPreviewZoom);
  }
  if (previewBox) {
    previewBox.addEventListener("wheel", event => {
      if (!previewThumb || currentSlideItem()?.kind !== "image") return;
      if (previewZoom <= 1 && event.deltaY > 0) return;
      event.preventDefault();
      setPreviewZoom(previewZoom + (event.deltaY < 0 ? 0.15 : -0.15));
    }, { passive: false });
    previewBox.addEventListener("dblclick", event => {
      if (event.target.closest?.(".preview-zoom-bar")) return;
      if (!previewThumb || currentSlideItem()?.kind !== "image") return;
      if (previewZoom > 1.05) resetPreviewZoom();
      else setPreviewZoom(2);
    });
    let panning = false;
    let panStartX = 0;
    let panStartY = 0;
    const endPan = pointerId => {
      if (!panning) return;
      panning = false;
      previewBox.classList.remove("is-panning");
      if (pointerId !== undefined) previewBox.releasePointerCapture?.(pointerId);
    };
    previewBox.addEventListener("pointerdown", event => {
      if (event.button !== 0 || previewZoom <= 1.001 || event.target.closest?.(".preview-zoom-bar")) return;
      panning = true;
      panStartX = event.clientX - previewPanX;
      panStartY = event.clientY - previewPanY;
      previewBox.classList.add("is-panning");
      previewBox.setPointerCapture?.(event.pointerId);
    });
    previewBox.addEventListener("pointermove", event => {
      if (!panning) return;
      previewPanX = event.clientX - panStartX;
      previewPanY = event.clientY - panStartY;
      applyPreviewZoomTransform();
    });
    previewBox.addEventListener("pointerup", event => endPan(event.pointerId));
    previewBox.addEventListener("pointercancel", event => endPan(event.pointerId));
    previewBox.addEventListener("lostpointercapture", () => endPan());
    window.addEventListener("blur", () => endPan());
  }

  window.addEventListener("keydown", event => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "l") {
      event.preventDefault();
      urlInput.focus();
      urlInput.select();
      return;
    }
    const activeEl = document.activeElement;
    const activeTag = activeEl?.tagName;
    const inRange = activeTag === "INPUT" && activeEl?.type === "range";
    if (!event.altKey && !event.ctrlKey && !event.metaKey
      && activeTag !== "TEXTAREA" && activeTag !== "SELECT" && (activeTag !== "INPUT" || inRange)) {
      const canZoom = Boolean(previewThumb && currentSlideItem()?.kind === "image");
      if (canZoom && (event.key === "+" || event.key === "=" || event.code === "NumpadAdd")) {
        event.preventDefault();
        setPreviewZoom(previewZoom + 0.15);
        return;
      }
      if (canZoom && (event.key === "-" || event.key === "_" || event.code === "NumpadSubtract")) {
        event.preventDefault();
        setPreviewZoom(previewZoom - 0.15);
        return;
      }
      if (canZoom && (event.key === "0" || event.key === "f" || event.key === "F")) {
        event.preventDefault();
        resetPreviewZoom();
        return;
      }
      if (canZoom && previewZoom > 1.001 && !inRange
        && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
        event.preventDefault();
        if (event.key === "ArrowUp") previewPanY += 32;
        else if (event.key === "ArrowDown") previewPanY -= 32;
        else if (event.key === "ArrowLeft") previewPanX += 32;
        else previewPanX -= 32;
        applyPreviewZoomTransform();
        return;
      }
      if (!inRange && event.key === "ArrowLeft") {
        event.preventDefault();
        stepSlide(-1);
      } else if (!inRange && event.key === "ArrowRight") {
        event.preventDefault();
        stepSlide(1);
      }
    }
  });

  function stepSlide(delta) {
    const items = currentPost?.items || [];
    if (busy || items.length <= 1) return;
    const nextIdx = Math.max(0, Math.min(items.length - 1, currentSlideIndex() + delta));
    if (nextIdx !== currentSlideIndex()) {
      slideSelect.value = String(nextIdx);
      renderFormatsForSlide(nextIdx);
    }
  }

  slideSelect.addEventListener("change", () => renderFormatsForSlide(currentSlideIndex()));
  if (prevSlideBtn) prevSlideBtn.addEventListener("click", () => stepSlide(-1));
  if (nextSlideBtn) nextSlideBtn.addEventListener("click", () => stepSlide(1));

  formatSelect.addEventListener("change", () => {
    const item = currentSlideItem();
    if (item) {
      const idx = currentSlideIndex();
      const fmt = selectedFormatForItem(item);
      if (previewTag) {
        const kindTag = fmt?.ext === "gif" || item.kind === "gif"
          ? "GIF"
          : item.kind === "video"
            ? "MP4"
            : (fmt?.ext || "JPG").toUpperCase();
        previewTag.textContent = (currentPost?.items?.length || 0) > 1
          ? `${idx + 1} / ${currentPost.items.length} ${kindTag}`
          : kindTag;
      }
      updatePrimaryButtonForSelection(item, idx);
      updateAudioAndFormatSpec(item);
    }
  });

  captionInput.addEventListener("input", updateActionButtons);

  if (copyStemBtn) {
    copyStemBtn.addEventListener("click", async () => {
      const stem = outputNamePreview?.textContent?.trim();
      if (!stem) return;
      try {
        await navigator.clipboard?.writeText?.(stem);
        copyStemBtn.textContent = "Copied";
        window.setTimeout(() => { copyStemBtn.textContent = "Copy"; }, 1200);
      } catch {
        setStatus("Could not write to clipboard.", "warn");
      }
    });
  }

  copyCaptionBtn.addEventListener("click", async () => {
    const text = captionInput.value;
    if (!text.trim()) return;
    try {
      await navigator.clipboard?.writeText?.(text);
      setStatus("Copied post caption to clipboard.", "ok");
    } catch {
      setStatus("Could not write to clipboard.", "warn");
    }
  });

  saveCaptionBtn.addEventListener("click", async () => {
    if (busy) return;
    const postPayload = currentPost || {
      videoId: "",
      username: "",
      displayName: "",
      postUrl: urlInput.value.trim(),
      createdAt: "",
      caption: captionInput.value,
      ffmpegAvailable: false,
      items: []
    };
    setBusy(true, "save");
    setStatus("Saving caption (.txt) to Downloads/TikTok-Now...");
    try {
      const savedPath = await invoke("save_caption_file", {
        post: postPayload,
        customCaption: captionInput.value,
        subfolder: null,
        fileStem: null
      });
      setStatus(`Saved caption to ${savedPath}`, "ok", true);
    } catch (error) {
      setStatus(error?.message || String(error), "err");
    } finally {
      setBusy(false);
      await drainQueuedContext();
    }
  });

  async function runDownload(mode) {
    if (busy || !currentPost) return;
    const itemIndex = currentSlideIndex();
    const formatId = formatSelect.value || null;
    activeDownloadMode = mode;
    setBusy(true, "download");
    if (mode === "audio") {
      setStatus("Extracting Audio Only (.m4a AAC) to Downloads/TikTok-Now...");
    } else if (mode === "bundle") {
      setProgress(0, currentPost.items.length);
      setStatus(`Saving full post (0/${currentPost.items.length} items)...`);
    } else {
      const item = currentSlideItem();
      const fmt = selectedFormatForItem(item);
      if (fmt?.ext === "gif") {
        setStatus("Converting video to animated GIF via FFmpeg...");
      } else {
        setStatus("Downloading selected media to Downloads/TikTok-Now...");
      }
    }
    try {
      const result = await invoke("download_post_selection", {
        post: currentPost,
        itemIndex,
        formatId,
        mode,
        customCaption: captionInput.value
      });
      if (mode === "bundle") {
        setProgress(currentPost.items.length, currentPost.items.length);
        const dest = result.subfolder ? `Downloads/TikTok-Now/${result.subfolder}` : "Downloads/TikTok-Now";
        const msg = `Saved ${result.savedPaths.length} file(s)${result.captionPath ? " + caption.txt" : ""} to ${dest}`;
        setStatus(result.warning ? `${msg}\nNote: ${result.warning}` : msg, result.warning ? "warn" : "ok", true);
      } else {
        const msg = `Saved ${result.savedPaths[0] || "file"}`;
        setStatus(result.warning ? `${msg}\nNote: ${result.warning}` : msg, result.warning ? "warn" : "ok", true);
      }
    } catch (error) {
      setStatus(error?.message || String(error), "err");
    } finally {
      activeDownloadMode = "";
      setBusy(false);
      await drainQueuedContext();
    }
  }

  downloadSelectedBtn.addEventListener("click", () => runDownload("single"));
  if (downloadAudioBtn) {
    downloadAudioBtn.addEventListener("click", () => runDownload("audio"));
  }
  downloadBundleBtn.addEventListener("click", () => runDownload("bundle"));

  if (typeof window.__TAURI__?.event?.listen === "function") {
    window.__TAURI__.event.listen("downloader-progress", event => {
      if (!busy || activeDownloadMode !== "bundle") return;
      const completed = Number(event?.payload?.completed);
      const total = Number(event?.payload?.total);
      if (Number.isFinite(completed) && Number.isFinite(total) && total > 0) {
        setProgress(completed, total);
        setStatus(`Saving full post (${completed}/${total} items)...`);
      }
    }).catch(() => {});
  }

  const openFolderHandler = () => {
    invoke("open_downloads_folder").catch(error => {
      setStatus(error?.message || String(error), "err");
    });
  };
  openFolderBtn.addEventListener("click", openFolderHandler);
  if (statusOpenBtn) statusOpenBtn.addEventListener("click", openFolderHandler);

  window.__tiktoknowLoadDownloaderContext = loadPendingContext;
  updateUrlControls();
  updateActionButtons();
  loadPendingContext();
})();
