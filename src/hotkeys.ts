/** Global (system-wide) shortcuts, shown in the Help menu and on the
 *  Displays page. Registered in src-tauri/src/lib.rs. */
export const HOTKEYS: [string, string][] = [
  ["Ctrl+Alt+1…5", "Select the active display"],
  ["Ctrl+Alt+→ / ←", "Next / previous verse or stanza on the active display"],
  ["Ctrl+Alt+B", "Blank / unblank the active display"],
  ["← / → (in output)", "Step that display's verses"],
  ["Tab / Shift+Tab (in output)", "Cycle between open fullscreen outputs"],
  ["M (in output)", "Move this output to the next screen"],
  ["Esc (in output)", "Close a fullscreen output"],
  ["Ctrl+1…5 (main window)", "Switch between the app pages"],
];
