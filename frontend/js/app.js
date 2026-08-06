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
  setInterval(() => {
    if (taglineText) {
      taglineText.style.opacity = '0';
      setTimeout(() => {
        captionIdx = (captionIdx + 1) % captions.length;
        taglineText.textContent = captions[captionIdx];
        taglineText.style.opacity = '1';
      }, 400);
    }
  }, 2000);

  function checkOnlineAndRedirect() {
    if (navigator.onLine) {
      statusText.textContent = 'Redirecting to TikTok...';
      setTimeout(() => {
        window.location.href = 'https://www.tiktok.com/';
      }, 2200);
    } else {
      statusText.textContent = 'No internet connection detected.';
      retryBtn.style.display = 'inline-block';
    }
  }

  retryBtn.addEventListener('click', () => {
    statusText.textContent = 'Checking connection...';
    retryBtn.style.display = 'none';
    setTimeout(checkOnlineAndRedirect, 1000);
  });

  checkOnlineAndRedirect();
});
