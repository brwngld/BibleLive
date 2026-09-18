import { HOTKEYS } from "../hotkeys";

/** Help → Keyboard shortcuts. The same list the Displays page shows. */
export default function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal">
        <h3>Keyboard shortcuts</h3>
        <div className="hotkey-list">
          {HOTKEYS.map(([keys, what]) => (
            <div className="hotkey-item" key={keys}>
              <code>{keys}</code>
              <span>{what}</span>
            </div>
          ))}
        </div>
        <div className="form-actions">
          <button className="primary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
