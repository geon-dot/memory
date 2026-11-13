// Which mascots there are and which picture shows each mood. No DOM access
// here, so this runs under `node --test` as well as in the webview. The app
// keeps the same list for its tray menu (src-tauri/src/mascot.rs).

/** The mascots one can choose, by id. 'chip' is 칩이, drawn in mascot.js. */
export const MASCOTS = ['chiikawa', 'hachiware', 'usagi', 'chip'];

/** The mascot shown when none has been chosen. */
export const DEFAULT_MASCOT = 'chiikawa';

// The picture file for each mood.
const PICTURE = { calm: 'idle', busy: 'busy', hot: 'hot' };

/** The mascot id in `text` (as saved or passed in), or the default one. */
export function mascotOf(text) {
  const name = typeof text === 'string' ? text.trim() : '';
  return MASCOTS.includes(name) ? name : DEFAULT_MASCOT;
}

/**
 * The picture of mascot `name` for each mood, as `{ calm, busy, hot }` paths
 * relative to the page. `null` for 칩이 and for unknown names, which have no
 * pictures.
 */
export function picturesOf(name) {
  if (name === 'chip' || !MASCOTS.includes(name)) return null;
  return Object.fromEntries(
    Object.entries(PICTURE).map(([mood, file]) => [mood, `mascot/${name}/${file}.png`]),
  );
}
