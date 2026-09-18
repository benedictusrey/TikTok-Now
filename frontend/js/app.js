document.addEventListener('DOMContentLoaded', () => {
  const statusText = document.getElementById('status-text');
  const retryBtn = document.getElementById('retry-btn');
  const taglineText = document.getElementById('tagline-text');

  const captions = [
    'Bringing the beat to your desktop 🎵',
    'Infinite scroll, zero clutter 🚀',
    'Ready for your daily dose of dopamine? ✨',
    'Fast, lightweight & powered by Rust ⚡',
    'Experience TikTok at peak performance 🎬'
  ];

  let captionIdx = 0;
  const taglineTimer = setInterval(() => {
    if (!taglineText) return;
    taglineText.style.opacity = '0';
    setTimeout(() => {
      captionIdx = (captionIdx + 1) % captions.length;
      taglineText.textContent = captions[captionIdx];
      taglineText.style.opacity = '1';
    }, 400);
  }, 2000);

  let launched = false;

  function launch() {
    if (launched) return;
    launched = true;
    clearInterval(taglineTimer);
    window.removeEventListener('online', launch);
    if (statusText) statusText.textContent = 'Redirecting to TikTok...';
    // v2.1.0: `location.replace` instead of `location.href` — the splash page is
    // dropped from the session history, so Back/Alt+Left never lands on a dead
    // splash that immediately re-redirects. Combined with the shorter delay
    // (350 ms instead of 2200 ms) this makes the launch feel near-instant
    // without losing the animated splash frame.
    setTimeout(() => {
      window.location.replace('https://www.tiktok.com/');
    }, 350);
  }

  function checkOnlineAndRedirect() {
    if (launched) return;
    if (navigator.onLine) {
      launch();
      return;
    }
    // Offline: show the retry affordance AND auto-recover the moment the OS
    // reports connectivity back (previously the user had to notice and click).
    if (statusText) statusText.textContent = 'No internet connection detected.';
    if (retryBtn) retryBtn.style.display = 'inline-block';
    window.addEventListener('online', launch);
  }

  if (retryBtn) {
    retryBtn.addEventListener('click', () => {
      if (launched) return;
      // Immediate re-evaluation: the OS 'online' event + this check replace the
      // old blind 1 s timeout, so a working connection is never left waiting.
      checkOnlineAndRedirect();
    });
  }

  checkOnlineAndRedirect();
});
