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
story.append(BULLET('<b>Content Engine</b> — the full KJV Bible (66 books) and a public-domain '
                    'hymn collection ship built in. Import your own songs and documents '
                    '(.txt / .docx) with license tracking.'))
story.append(BULLET('<b>Display Engine</b> — five independent Display Slots. Each slot outputs '
                    'a fullscreen window on any monitor, with its own font, size and colors, '
                    'AUTO/MANUAL/LOCK mode, and display profiles.'))
story.append(BULLET('<b>Voice Engine</b> — listens through a microphone or mixer, transcribes '
                    'locally with Whisper (no cloud), and detects Bible references '
                    '("John 3:16", "John chapter three verse sixteen") and quoted verses.'))
story.append(BULLET('<b>Service Control</b> — start a service, approve AI suggestions, blank '
                    'every screen in an emergency, and keep an automatic log of everything '
                    'projected.'))
story.append(Spacer(1, 6))
story.append(P(
    'Everything the AI does is a <b>suggestion</b>. Nothing reaches a screen unless a '
    'rule allows it (Manual / Assisted / Automatic), and the operator can always take '
    'over. The AI only ever matches content that exists in the library — it never '
    'generates Scripture.', muted=False))

# ---- 2. Install ---------------------------------------------------------------
story.append(H1('2. Installing on Another PC'))
story.append(P('Requirements: Windows 10 or 11 (64-bit). Nothing else: no internet and no '
               'extra runtimes. Use the <b>all-in-one package</b> (recommended) or the '
               'plain installer.'))
story.append(table([
    ['Item', 'Size', 'Purpose'],
    ['BibleLive_0.1.0_x64-setup.exe', '5 MB', 'The application installer'],
    ['ggml-base.en.bin', '148 MB', 'Voice model — needed only for speech features'],
    ['Install-BibleLive.cmd', '2 KB', 'One-click: installs the app, then copies the voice model'],
], [34, 10, 56]))
story.append(Spacer(1, 6))
story.append(H2('All-in-one (recommended)'))
story.append(BULLET('Copy the whole <b>BibleLive-FullSetup</b> folder to the target PC.'))
story.append(BULLET('Double-click <b>Install-BibleLive.cmd</b>. It installs the app silently '
                    'and places the voice model in the right folder automatically.'))
story.append(BULLET('Launch <b>BibleLive</b> from the Start menu. The KJV and hymns are seeded '
                    'on first start.'))
story.append(H2('Voice-only extra step (plain installer)'))
story.append(P('If you install without the script, copy ggml-base.en.bin to '
               '<b>%APPDATA%\\BibleLive\\models\\</b> (create the folder if needed). '
               'Without it, everything works except listening and transcription.'))
story.append(H2('Moving your library too'))
story.append(P('Your imported documents, display profiles and service history live in '
               '<b>%APPDATA%\\BibleLive\\biblelive.db</b>. Copy that one file to the same '
               'location on the new PC to clone your setup exactly.'))

# ---- 3. Library ---------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('3. The Library Tab'))
story.append(P('Browse and search everything installed. Filter by type (Bible, Hymn, Song, '
               'Book, Document), sort A-Z / Z-A / by date added, or use <b>canonical order</b> '
               '(Genesis to Revelation). When the Bible filter is active, Old Testament and '
               'New Testament sub-filters appear.'))
story.append(H2('Searching'))
story.append(P('The search box searches <b>all content at once</b>. Type a reference '
               '("John 3:16"), a quotation ("for God so loved the world"), or a hymn line '
               '("amazing grace"). Results show highlighted snippets; click one to read the '
               'full chapter or song.'))
story.append(H2('Importing your own content'))
story.append(BULLET('Click <b>+ Import / Add</b>, then paste text or pick a .txt / .docx file.'))
story.append(BULLET('Blank lines separate stanzas or sections; lines like "[Verse 1]" or '
                    '"Chorus:" become section labels that the display engine can page through.'))
story.append(BULLET('Choose a license. Copyrighted or unknown-license content is '
                    'automatically marked <b>private</b> as a safeguard.'))

# ---- 4. Displays ---------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('4. The Displays Tab'))
story.append(P('Five Display Slots, each independently controlled. A slot is a fullscreen '
               'black window that shows scripture, lyrics, images or video on whatever '
               'monitor you assign it to.'))
story.append(H2('Per-slot controls'))
story.append(BULLET('<b>Open output</b> — creates the fullscreen window on the chosen monitor '
                    '(falls back to the primary monitor with a warning if it is missing).'))
story.append(BULLET('<b>AUTO / MANUAL / LOCK</b> — whether AI suggestions may target this slot; '
                    'LOCK keeps content on screen no matter what.'))
story.append(BULLET('<b>Scripture / Lyrics / Media</b> — put content on the slot. Picking a '
                    'verse loads its <b>whole chapter</b>; picking a song loads all its stanzas, '
                    'so Prev/Next walks through them.'))
story.append(BULLET('<b>Style</b> — font family (12 choices), text size, text color and '
                    'background color. Text auto-shrinks to fit any screen, so nothing ever '
                    'overflows. Styles are saved into profiles.'))
story.append(BULLET('<b>Profiles</b> — save all five slots\u2019 monitor + mode + style layout '
                    'under a name ("Sunday Service") and re-apply it in one click.'))
story.append(H2('Keys inside a fullscreen output'))
story.append(table([
    ['Key', 'Action'],
    ['Left / Right arrows', 'Previous / next verse or stanza on that display'],
    ['Tab / Shift+Tab', 'Switch keyboard focus between open fullscreen outputs'],
    ['M', 'Move this output to the next connected screen'],
    ['Esc', 'Close the output (its content stays on the slot)'],
], [30, 70]))

# ---- 5. Hotkeys -----------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('5. Global Hotkeys'))
story.append(P('These work system-wide, even when a fullscreen output has focus. They act on '
               'the <b>active display</b> \u2014 select it with Ctrl+Alt+1 to 5 (the card shows a '
               'gold star; the output shows a brief blue banner). Works with both the number '
               'row and the numeric keypad.'))
story.append(table([
    ['Keys', 'Action'],
    ['Ctrl+Alt+1 \u2026 5', 'Select the active display'],
    ['Ctrl+Alt+Right / Left', 'Next / previous verse or stanza on the active display'],
    ['Ctrl+Alt+B', 'Blank / unblank the active display'],
], [30, 70]))
story.append(P('If a combination clashes with other software, close that software during the '
               'service, or ask for the bindings to be made configurable.', muted=True))

# ---- 6. Voice --------------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('6. The Voice Tab'))
story.append(H2('One-time audio setup'))
story.append(BULLET('<b>Audio source</b> — pick the microphone the preacher uses. A USB '
                    'microphone works; a Bluetooth headset works too (use its "Hands-Free" '
                    'entry; connect it before opening the tab). Best results with larger '
                    'churches: a speech-only <b>aux/bus output from the mixer</b> fed into the PC.'))
story.append(BULLET('<b>Source contains</b> — "Speech only" for a dedicated mic or aux channel; '
                    '"Mixed church audio" if the PC receives the full worship mix.'))
story.append(BULLET('<b>Sensitivity</b> — lower = more sensitive. Raise it if silence or '
                    'music triggers false detections.'))
story.append(H2('The audio test — always run it before a service'))
story.append(P('Click <b>Run audio test</b> and say "Testing BibleLive". After 8 seconds the '
               'panel shows signal level, whether speech was detected, and what was '
               'recognized. This catches the classic problem of Windows listening to the '
               'wrong device.'))
story.append(H2('Listening modes'))
story.append(table([
    ['Mode', 'Behavior'],
    ['Manual', 'Suggestions queue up; you control everything'],
    ['Assisted (default)', 'Detected verses appear as cards with SHOW / IGNORE'],
    ['Automatic', 'High-confidence matches are pushed to displays without approval'],
], [24, 76]))
story.append(P('During a service the system listens continuously but only works when it hears '
               'speech. A mute button is one click away. Everything is processed locally; '
               'no audio ever leaves the PC.', muted=True))

# ---- 7. Live Service ---------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('7. The Live Service Tab'))
story.append(P('The control room for running an actual service.'))
story.append(BULLET('<b>Start service</b> — name it (e.g. "Sunday Morning") or leave blank for '
                    'an automatic date. A timer tracks the service.'))
story.append(BULLET('<b>Auto-recorded log</b> — every verse, song, image or video shown on any '
                    'display is recorded with a timestamp. After the service it is saved; '
                    'History lets you reopen any past service\u2019s log.'))
story.append(BULLET('<b>AI suggestions</b> — when the preacher says a reference, a card appears '
                    'with the verse and a confidence score. SHOW puts it on Display 1; IGNORE '
                    'dismisses it.'))
story.append(BULLET('<b>Display strip</b> — five tiles showing each slot\u2019s live status and '
                    'current content. Click a tile to make it the hotkey target.'))
story.append(BULLET('<b>EMERGENCY BLANK ALL</b> — blacks out every display at once; Restore all '
                    'brings everything back.'))

# ---- 8. A typical service ------------------------------------------------------------
story.append(CondPageBreak(120))
story.append(H1('8. A Typical Service, Step by Step'))
story.append(table([
    ['Step', 'Action'],
    ['1', 'Displays tab: open outputs for the slots you need (Display 1 minimum). Load a '
          'opening scripture or apply a saved profile.'],
    ['2', 'Voice tab: run the audio test once. Then click Start listening.'],
    ['3', 'Live Service tab: Start service. Keep this tab open \u2014 it is your control room.'],
    ['4', 'During the sermon: when a suggestion card appears, click SHOW (or press nothing '
          'and let Automatic mode work). Use Ctrl+Alt+Right to walk through the chapter.'],
    ['5', 'Worship: Lyrics picker on another display; Ctrl+Alt+2 to target it, arrows to '
          'change stanzas.'],
    ['6', 'Emergency: Ctrl+Alt+B blanks the active display; EMERGENCY BLANK ALL blacks out '
          'every screen.'],
    ['7', 'Afterwards: End service. The full log is saved under History.'],
], [8, 92]))

# ---- 9. Files & troubleshooting --------------------------------------------------------
story.append(PageBreak())
story.append(H1('9. File Locations and Troubleshooting'))
story.append(H2('Where things live'))
story.append(table([
    ['Location', 'Contents'],
    ['%APPDATA%\\BibleLive\\biblelive.db', 'Your whole library, profiles and service history'],
    ['%APPDATA%\\BibleLive\\models\\ggml-base.en.bin', 'The voice model (148 MB)'],
    ['Display outputs', 'Open from the Displays tab; fullscreen windows on your monitors'],
], [40, 60]))
story.append(H2('Common problems'))
story.append(table([
    ['Problem', 'Fix'],
    ['Voice tab says model not found', 'Copy ggml-base.en.bin into %APPDATA%\\BibleLive\\models\\ '
     '(the all-in-one installer script does this for you).'],
    ['No speech detected in the audio test', 'Wrong input device selected, or input muted in '
     'Windows sound settings. Check the device shows a level meter in Windows while you speak.'],
    ['Bluetooth microphone missing', 'Connect it first, then reopen the Voice tab. Use the '
     '"Hands-Free" entry, not "Stereo". If still missing: Control Panel > Devices and '
     'Printers > device Properties > Services > enable Hands-Free Telephony.'],
    ['Hotkeys do nothing', 'They are global while BibleLive runs \u2014 check no other app has '
     'claimed them, and that a display is active (gold star). Digits above the letters and '
     'the numpad both work.'],
    ['A display shows only black', 'That is the blank state. Press Unblank on its card, or '
     'Ctrl+Alt+B. Content stays on the slot.'],
    ['Preacher talks but nothing happens', 'Check the mode is not Manual-with-stop, that '
     'listening is on (red dot), and that he is actually quoting a verse the library '
     'contains. Partial quotes and paraphrases arrive in a later version.'],
], [30, 70]))
story.append(Spacer(1, 10))
story.append(P('BibleLive v1.0 \u2014 offline-first, local-data-first. Your library belongs to '
               'your church, on your machine.', muted=True))

doc = SimpleDocTemplate(
    'body.pdf', pagesize=A4,
    leftMargin=MARGIN, rightMargin=MARGIN, topMargin=50, bottomMargin=48,
    title='BibleLive User Manual', author='Z.ai',
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
