// 칩이, the widget's mascot. Everything that draws and moves it is here and
// in mascot.css; the rest of the widget only mounts it and tells it the mood.
// Another mascot would replace these two files and keep `mountMascot`.

// The parts of every face are all in the one drawing; mascot.css shows the
// ones that belong to the mood on the host element (`data-mood`).
const ART = `
  <svg class="chip" viewBox="0 0 64 64" aria-hidden="true">
    <ellipse class="ground" cx="32" cy="59.5" rx="16" ry="2.4" />
    <g class="whole">
      <g class="steam">
        <path d="M24 8.5q-3-2.8 0-5.6" />
        <path d="M40 8.5q3-2.8 0-5.6" />
      </g>
      <g class="pins">
        <rect x="4.5" y="21" width="10" height="5" rx="2.5" />
        <rect x="4.5" y="30" width="10" height="5" rx="2.5" />
        <rect x="4.5" y="39" width="10" height="5" rx="2.5" />
        <rect x="49.5" y="21" width="10" height="5" rx="2.5" />
        <rect x="49.5" y="30" width="10" height="5" rx="2.5" />
        <rect x="49.5" y="39" width="10" height="5" rx="2.5" />
        <rect x="21.5" y="48" width="7" height="10" rx="3.5" />
        <rect x="35.5" y="48" width="7" height="10" rx="3.5" />
      </g>
      <rect class="shell" x="11" y="11" width="42" height="42" rx="12" />
      <path class="notch" d="M27.5 11.4a4.5 4.5 0 0 0 9 0" />
      <ellipse class="blush" cx="19.8" cy="38" rx="3.1" ry="1.9" />
      <ellipse class="blush" cx="44.2" cy="38" rx="3.1" ry="1.9" />
      <g class="eyes open">
        <rect x="23.2" y="27.6" width="3.6" height="7" rx="1.8" />
        <rect x="37.2" y="27.6" width="3.6" height="7" rx="1.8" />
      </g>
      <g class="eyes shut">
        <path d="M22.6 28.4l4.6 2.7-4.6 2.7" />
        <path d="M41.4 28.4l-4.6 2.7 4.6 2.7" />
      </g>
      <g class="brows">
        <path d="M21.6 23.6l5.6 1.8" />
        <path d="M42.4 23.6l-5.6 1.8" />
      </g>
      <path class="mouth m-calm" d="M28.6 38.2q3.4 3.2 6.8 0" />
      <ellipse class="mouth m-busy" cx="32" cy="39.6" rx="1.9" ry="2.3" />
      <path class="mouth m-hot" d="M26.8 40.4q2.6-2.8 5.2 0t5.2 0" />
      <path class="sweat" d="M54.5 9.5q3.4 4.4 0 6.4q-3.4-2 0-6.4z" />
    </g>
  </svg>
`;

// 칩이 blinks on a timer aimed at the next change (eyes open for a few
// seconds of uneven length, shut for one short cel), not on a frame loop.
const BLINK_OPEN_MS = 2400;
const BLINK_JITTER_MS = 1600;
const BLINK_SHUT_MS = 130;

/**
 * Draws the mascot into `host` and returns `{ setMood }`, where a mood is
 * 'calm', 'busy' or 'hot'. With `idleMotion` off the mascot still changes
 * face with the mood but holds still.
 */
export function mountMascot(host, { idleMotion = true } = {}) {
  host.innerHTML = ART;
  host.dataset.mood = 'calm';
  const chip = host.querySelector('.chip');
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
  let blinkTimer = 0;

  function blinkLater() {
    blinkTimer = setTimeout(() => {
      chip.classList.add('blink');
      blinkTimer = setTimeout(() => {
        chip.classList.remove('blink');
        blinkLater();
      }, BLINK_SHUT_MS);
    }, BLINK_OPEN_MS + Math.random() * BLINK_JITTER_MS);
  }

  // Movement runs only while the page is showing and motion is welcome.
  function syncMotion() {
    const still = !idleMotion || document.hidden || reducedMotion.matches;
    host.classList.toggle('still', !idleMotion);
    host.classList.toggle('paused', document.hidden);
    clearTimeout(blinkTimer);
    chip.classList.remove('blink');
    if (!still) blinkLater();
  }
  document.addEventListener('visibilitychange', syncMotion);
  reducedMotion.addEventListener('change', syncMotion);
  syncMotion();

  return {
    setMood(mood) {
      host.dataset.mood = mood;
    },
  };
}
