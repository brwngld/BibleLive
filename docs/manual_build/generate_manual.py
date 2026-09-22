"""Generate the BibleLive User Manual body PDF (no cover — cover merged later)."""
from reportlab.lib.pagesizes import A4
from reportlab.lib.units import mm
from reportlab.lib import colors
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.enums import TA_LEFT
from reportlab.platypus import (
    SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, PageBreak,
    KeepTogether, CondPageBreak,
)

# Palette (from pdf.py palette.cascade)
PAGE_BG      = colors.HexColor('#f4f4f3')
HEADER_FILL  = colors.HexColor('#746b4e')
BORDER       = colors.HexColor('#c2bdab')
ACCENT       = colors.HexColor('#4c26bc')
TEXT_PRIMARY = colors.HexColor('#242321')
TEXT_MUTED   = colors.HexColor('#7e7c74')
SEM_SUCCESS  = colors.HexColor('#417954')
TABLE_STRIPE = colors.HexColor('#f0efed')

PAGE_W, PAGE_H = A4
MARGIN = 46
AVAIL = PAGE_W - 2 * MARGIN

def H1(text):
    s = ParagraphStyle('H1', fontName='Helvetica-Bold', fontSize=17, leading=22,
                       textColor=HEADER_FILL, spaceBefore=18, spaceAfter=8)
    return Paragraph(text, s)

def H2(text):
    s = ParagraphStyle('H2', fontName='Helvetica-Bold', fontSize=12.5, leading=16,
                       textColor=TEXT_PRIMARY, spaceBefore=12, spaceAfter=5)
    return Paragraph(text, s)

def P(text, muted=False):
    s = ParagraphStyle('P', fontName='Helvetica', fontSize=9.8, leading=14.5,
                       textColor=TEXT_MUTED if muted else TEXT_PRIMARY, spaceAfter=6)
    return Paragraph(text, s)

def BULLET(text):
    s = ParagraphStyle('B', fontName='Helvetica', fontSize=9.8, leading=14.5,
                       textColor=TEXT_PRIMARY, leftIndent=14, bulletIndent=4, spaceAfter=3)
    return Paragraph(text, s, bulletText='\u2022')

def table(rows, widths, header=True):
    data = []
    for i, row in enumerate(rows):
        style = ParagraphStyle(
            f'c{i}', fontName='Helvetica-Bold' if (header and i == 0) else 'Helvetica',
            fontSize=9, leading=12.5,
            textColor=colors.white if (header and i == 0) else TEXT_PRIMARY)
        data.append([Paragraph(c, style) for c in row])
    total = sum(widths)
    widths = [w / total * AVAIL for w in widths]
    t = Table(data, colWidths=widths, hAlign='CENTER', repeatRows=1 if header else 0)
    cmds = [
        ('VALIGN', (0, 0), (-1, -1), 'TOP'),
        ('GRID', (0, 0), (-1, -1), 0.5, BORDER),
        ('LEFTPADDING', (0, 0), (-1, -1), 6),
        ('RIGHTPADDING', (0, 0), (-1, -1), 6),
        ('TOPPADDING', (0, 0), (-1, -1), 4),
        ('BOTTOMPADDING', (0, 0), (-1, -1), 4),
    ]
    if header:
        cmds.append(('BACKGROUND', (0, 0), (-1, 0), HEADER_FILL))
        for r in range(1, len(rows)):
            if r % 2 == 0:
                cmds.append(('BACKGROUND', (0, r), (-1, r), TABLE_STRIPE))
    t.setStyle(TableStyle(cmds))
    return t

story = []

# ---- 1. What is BibleLive ---------------------------------------------------
story.append(H1('1. What BibleLive Does'))
story.append(P(
    'BibleLive is a complete digital church media assistant for Windows. It runs '
    'entirely offline on one PC and combines four engines under a single Live '
    'Service control room:'))
story.append(BULLET('<b>Content Engine</b> — three full Bible translations ship built in '
                    '(KJV, ASV and the World English Bible — 66 books each), plus a '
                    'public-domain hymn collection and editable starter slide sets. '
                    'Import your own songs and documents (.txt / .docx) with license '
                    'tracking, and create custom slides (welcome screens, announcements, '
                    'sermon points).'))
story.append(BULLET('<b>Display Engine</b> — five independent Display Slots. Each slot outputs '
                    'a fullscreen window on any monitor, with its own theme (background '
                    'image, font, colors, alignment, transition effects), an optional '
                    '<b>second Bible version</b> shown side by side, AUTO/MANUAL/LOCK mode, '
                    'saved themes and display profiles.'))
story.append(BULLET('<b>Voice Engine</b> — listens through a microphone or mixer, transcribes '
                    'locally with Whisper (no cloud, no audio ever leaves the PC), and '
                    'detects Bible references ("John 3:16", "John chapter three verse '
                    'sixteen") and quoted verses — even mid-sentence while someone reads '
                    'continuously.'))
story.append(BULLET('<b>Service Control</b> — plan the service in a queue, approve AI '
                    'suggestions, show lower-third announcements, blank every screen in '
                    'an emergency, and keep an automatic log of everything projected.'))
story.append(Spacer(1, 6))
story.append(P(
    'Everything the AI does is a <b>suggestion</b>. Nothing reaches a screen unless a '
    'rule allows it (Manual / Assisted / Automatic), and the operator can always take '
    'over — Automatic mode keeps a 10-second Undo button. The AI only ever matches '
    'content that exists in the library — it never generates Scripture.', muted=False))

# ---- 2. The app layout --------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('2. The App at a Glance'))
story.append(P('The window has a classic desktop layout: a <b>menu row</b> (File / View / '
               'Settings / Tools / Help) above a <b>tab strip</b> with five pages. '
               'Ctrl+1 … Ctrl+5 switch pages.'))
story.append(table([
    ['Page', 'What it is for'],
    ['📚 Library', 'Browse, search, read and import everything: Bibles, hymns, songs, '
                   'books, documents and your custom slides.'],
    ['🖼 Displays', 'The five output slots: monitors, modes, themes, second version, '
                    'profiles, and the content pickers.'],
    ['🎙 Voice', 'The live listening console: start/stop, level meter, transcript feed. '
                 '(One-time audio setup lives in ⚙ Settings.)'],
    ['🎛 Live Service', 'The control room during a service: session timer, suggestions, '
                        'queue, announcements, display strip, emergency blank.'],
    ['⚙ Settings', 'One-time configuration: audio & voice, Automatic-mode target, '
                   'reader animation, data tools, about.'],
], [22, 78]))

# ---- 3. Library ---------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('3. The Library Tab'))
story.append(P('Browse and search everything installed. Filter by type (Bible, Hymn, Song, '
               'Slide, Book, Document), sort A-Z / Z-A / by date added, or use <b>canonical '
               'order</b> (Genesis to Revelation). When the Bible filter is active, version '
               'tabs (KJV / ASV / WEB) and Old/New Testament sub-filters appear.'))
story.append(H2('Searching'))
story.append(P('The search box searches <b>all content at once</b>. Type a reference '
               '("John 3:16"), a quotation ("for God so loved the world"), or a hymn line '
               '("amazing grace"). Results show highlighted snippets; click one to read the '
               'full chapter or song.'))
story.append(H2('The Bible reader'))
story.append(P('Click any Bible book to read it chapter by chapter: jump to any book/chapter, '
               'step with the arrow buttons, and optionally enable a fade or slide animation '
               'on chapter changes (⚙ Settings → Bible reader).'))
story.append(H2('Custom slides'))
story.append(BULLET('<b>📝 New slide</b> creates a slide set: give it a title, add slides, '
                    'each with an optional heading and text (one line per displayed line).'))
story.append(BULLET('By default lines <b>reveal one by one</b> on the projector (like '
                    'bullet-point builds). Untick "Reveal lines one by one" to always show '
                    'every line and step whole slides — right for welcome screens.'))
story.append(BULLET('Three starter sets (Welcome, Announcements, Sermon points) ship built '
                    'in — edit them for your church.'))
story.append(H2('Importing your own content'))
story.append(BULLET('Click <b>+ Import / Add</b> (or File → Import content…), then paste text '
                    'or pick a .txt / .docx file.'))
story.append(BULLET('Blank lines separate stanzas or sections; lines like "[Verse 1]" or '
                    '"Chorus:" become section labels that the display engine can page through.'))
story.append(BULLET('Choose a license. Copyrighted or unknown-license content is '
                    'automatically marked <b>private</b> as a safeguard.'))

# ---- 4. Displays ---------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('4. The Displays Tab'))
story.append(P('Five Display Slots, each independently controlled. A slot is a fullscreen '
               'window that shows scripture, slides, lyrics, images or video on whatever '
               'monitor you assign it to.'))
story.append(H2('Per-slot controls'))
story.append(BULLET('<b>Open output</b> — creates the fullscreen window on the chosen monitor '
                    '(falls back to the primary monitor with a warning if it is missing).'))
story.append(BULLET('<b>AUTO / MANUAL / LOCK</b> — whether automation may target this slot. '
                    'LOCK keeps content on screen no matter what — even a pinned '
                    'Automatic-mode target is respected.'))
story.append(BULLET('<b>⧉ Second version</b> — show the same scripture in a second Bible '
                    'version (KJV / ASV / WEB), side by side. Stepping advances both '
                    'columns in lockstep.'))
story.append(BULLET('<b>📖 Scripture…</b> — pick a verse by browsing (version → book → '
                    'chapter → verse → optional <b>"to verse" range</b>) or by searching. '
                    'Picking a verse loads its whole chapter, so Prev/Next walks it.'))
story.append(BULLET('<b>🎵 Lyrics…</b> / <b>📝 Slide…</b> — put a song stanza or a slide from '
                    'your custom sets on screen; Prev/Next walks the whole set.'))
story.append(BULLET('<b>🖼 Media…</b> — show an image or video full screen.'))
story.append(H2('Themes — how a display looks'))
story.append(P('Each card\u2019s 🔠 style panel controls that display\u2019s look: font, size, '
               'text and background colors, a <b>background image</b>, text alignment, a '
               'legibility shadow, and a <b>transition</b> (none / fade / slide) played when '
               'the content changes. Five built-in presets (Classic, Cathedral, Modern, '
               'Lantern, Bulletin) set a whole look in one click.'))
story.append(BULLET('<b>Saved theme templates</b> — "Save this look as…" captures the whole '
                    'style under a name; apply it to any display from any slot\u2019s style '
                    'panel. Great for seasons (Christmas vs. regular).'))
story.append(BULLET('<b>Display profiles</b> (File menu or Displays tab) — save all five '
                    'slots\u2019 monitor + mode layout under a name and re-apply it in one '
                    'click. Profiles are the room setup; templates are the look.'))
story.append(H2('Keys inside a fullscreen output'))
story.append(table([
    ['Key', 'Action'],
    ['Left / Right arrows', 'Previous / next verse, stanza or slide on that display'],
    ['Tab / Shift+Tab', 'Switch keyboard focus between open fullscreen outputs'],
    ['M', 'Move this output to the next connected screen'],
    ['Esc', 'Close the output (its content stays on the slot)'],
], [30, 70]))

# ---- 5. Hotkeys -----------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('5. Keyboard Shortcuts'))
story.append(P('Global shortcuts work system-wide, even when a fullscreen output has focus. '
               'They act on the <b>active display</b> — select it with Ctrl+Alt+1 to 5 (the '
               'card shows a gold star; the output shows a brief blue banner). Works with '
               'both the number row and the numeric keypad.'))
story.append(table([
    ['Keys', 'Action'],
    ['Ctrl+Alt+1 \u2026 5', 'Select the active display'],
    ['Ctrl+Alt+Right / Left', 'Next / previous verse, stanza or slide on the active display'],
    ['Ctrl+Alt+B', 'Blank / unblank the active display'],
    ['Ctrl+1 \u2026 5 (main window)', 'Switch between the app pages'],
], [30, 70]))
story.append(P('The full list is always available under Help → Keyboard shortcuts.', muted=True))

# ---- 6. Settings & Voice ----------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('6. Settings & the Voice Console'))
story.append(H2('⚙ Settings — one-time setup'))
story.append(BULLET('<b>🎙 Audio & Voice</b> — audio source (with device re-scan), whether it '
                    'carries speech only or the mixed church audio, speech sensitivity '
                    '(auto-adapts to the room), the transcription model (Base = most '
                    'accurate; Tiny = ~4× faster, more mistakes), and the live matching '
                    'window. The <b>audio test</b> (8 s listen) and the 🔬 <b>diagnostics '
                    'report</b> live here too — run the test before every service.'))
story.append(BULLET('<b>🖥 Displays</b> — the Automatic-mode target: the first display set to '
                    'AUTO, or always a specific display 1–5 (LOCK is never taken over). '
                    'Also changeable on the Live page mid-service.'))
story.append(BULLET('<b>📖 Bible reader</b> — chapter-change animation in the Library.'))
story.append(BULLET('<b>💾 Data</b> — backup/restore guidance and a button to open the data '
                    'folder (%APPDATA%\\BibleLive).'))
story.append(H2('🎙 Voice tab — the live console'))
story.append(P('Pick the listening mode, press <b>Start listening</b>, and watch the input '
               'level and the live transcript. Detected scripture appears as suggestion '
               'cards here and on the Live page.'))
story.append(table([
    ['Mode', 'Behavior'],
    ['Manual', 'Suggestions queue up; you control everything'],
    ['Assisted (default)', 'Detected verses appear as cards with SHOW / IGNORE'],
    ['Automatic', 'Verified high-confidence matches project themselves onto the target '
                  'display, with a 10-second UNDO button'],
], [24, 76]))
story.append(P('Scripture is matched continuously from partial speech — the reader does not '
               'need to pause. Everything is processed locally; no audio ever leaves the PC.',
               muted=True))

# ---- 7. Live Service ---------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('7. The Live Service Tab'))
story.append(P('The control room for running an actual service.'))
story.append(BULLET('<b>Start service</b> — name it (e.g. "Sunday Morning") or leave blank for '
                    'an automatic date. A timer tracks the service.'))
story.append(BULLET('<b>AI suggestions</b> — when a verse is detected, a card appears with the '
                    'verse text (preview), a confidence score, and SHOW / IGNORE. In '
                    'Automatic mode cards show ⚡ auto → Display N with an UNDO button for '
                    '10 seconds.'))
story.append(BULLET('<b>📋 Queue</b> — plan the service before it starts: type references '
                    '("John 3:16-18") or pick slide/song sets, reorder with ▲▼, and press '
                    'Show to project an entry onto the chosen display. The queue stays '
                    'until you clear it.'))
story.append(BULLET('<b>📣 Announcement</b> — a lower-third banner ("Lunch is served in the '
                    'hall") over any display for 10 s / 30 s / 1 min / 5 min / until '
                    'hidden. It never touches the content behind it.'))
story.append(BULLET('<b>Display strip</b> — five tiles showing each slot\u2019s live status and '
                    'current content. Click a tile to make it the hotkey target.'))
story.append(BULLET('<b>EMERGENCY BLANK ALL</b> — blacks out every display at once; Restore all '
                    'brings everything back.'))
story.append(BULLET('<b>Auto-recorded log</b> — every verse, song, slide, image or video shown '
                    'on any display is recorded with a timestamp and saved with the session; '
                    'History reopens any past service\u2019s log.'))

# ---- 8. A typical service ------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('8. A Typical Service, Step by Step'))
story.append(table([
    ['Step', 'Action'],
    ['Before', 'Displays tab: open outputs, assign monitors, apply a profile and themes. '
               'Live Service tab: fill the queue with today\u2019s scriptures and slides.'],
    ['1', 'Run the audio test once (⚙ Settings). Then Voice tab: Start listening.'],
    ['2', 'Live Service tab: Start service. Keep this tab open — it is your control room.'],
    ['3', 'Opening: Show the first queue item (welcome slide / call to worship scripture).'],
    ['4', 'During the sermon: suggestion cards appear as verses are quoted — SHOW them, or '
          'let Automatic mode project with Undo. Ctrl+Alt+Right walks the chapter; a '
          'second-version display shows both translations.'],
    ['5', 'Worship: lyrics on another display; Ctrl+Alt+2 to target it, arrows to change '
          'stanzas.'],
    ['6', 'Announcements: type the lunch notice, Show banner — it disappears by itself.'],
    ['7', 'Emergency: Ctrl+Alt+B blanks the active display; EMERGENCY BLANK ALL blacks out '
          'every screen.'],
    ['8', 'Afterwards: End service. The full log is saved under History.'],
], [10, 90]))

# ---- 9. Files & troubleshooting --------------------------------------------------------
story.append(PageBreak())
story.append(H1('9. Files, Backup and Troubleshooting'))
story.append(H2('Where things live'))
story.append(table([
    ['Location', 'Contents'],
    ['%APPDATA%\\BibleLive\\biblelive.db', 'Your whole library, slides, themes, settings and '
     'service history'],
    ['%APPDATA%\\BibleLive\\models\\ggml-base.en.bin', 'The voice model (also crash.log and '
     'diagnostics live in this folder)'],
], [40, 60]))
story.append(H2('Backup and restore'))
story.append(P('<b>File → Backup data…</b> saves everything above to a single JSON file — use '
               'it before reinstalling or when moving to the church PC. <b>File → Restore '
               'from backup…</b> puts it all back (it asks for confirmation first).'))
story.append(H2('Common problems'))
story.append(table([
    ['Problem', 'Fix'],
    ['Settings says model not found', 'The installer bundles the models; if the folder was '
     'cleaned, copy ggml-base.en.bin into %APPDATA%\\BibleLive\\models\\ (download links are '
     'in the README).'],
    ['No speech detected in the audio test', 'Wrong input device selected, or input muted in '
     'Windows sound settings. Check the device shows a level meter in Windows while you speak.'],
    ['Bluetooth microphone missing', 'Connect it first, then use ↻ Re-scan devices in '
     '⚙ Settings. Use the "Hands-Free" entry, not "Stereo".'],
    ['Hotkeys do nothing', 'They are global while BibleLive runs — check no other app has '
     'claimed them, and that a display is active (gold star). Digits above the letters and '
     'the numpad both work.'],
    ['A display shows only black', 'That is the blank state. Press Unblank on its card, or '
     'Ctrl+Alt+B. Content stays on the slot.'],
    ['Preacher talks but nothing happens', 'Check listening is on and the mode is not Manual. '
     'He must quote a verse the library contains — say the reference plainly ("John chapter '
     'three verse sixteen") or read the words of a bundled translation.'],
    ['Automatic mode projected the wrong verse', 'Press UNDO on the card within 10 seconds — '
     'the previous content is restored exactly, and the verse is marked ignored.'],
    ['A display set to LOCK changed', 'It should not — LOCK always protects a slot, even from '
     'a pinned Automatic target. Check the second-version and style settings are on the '
     'slot you think they are.'],
], [30, 70]))
story.append(Spacer(1, 10))
story.append(P('BibleLive — offline-first, local-data-first. Your library belongs to '
               'your church, on your machine.', muted=True))

doc = SimpleDocTemplate(
    'body.pdf', pagesize=A4,
    leftMargin=MARGIN, rightMargin=MARGIN, topMargin=50, bottomMargin=48,
    title='BibleLive User Manual', author='BibleLive',
)
def on_page(canvas, doc_):
    canvas.saveState()
    canvas.setFont('Helvetica', 8)
    canvas.setFillColor(TEXT_MUTED)
    canvas.drawString(MARGIN, 28, 'BibleLive User Manual')
    canvas.drawRightString(PAGE_W - MARGIN, 28, f'Page {doc_.page + 1}')
    canvas.restoreState()

doc.build(story, onFirstPage=on_page, onLaterPages=on_page)
print('body.pdf built')
