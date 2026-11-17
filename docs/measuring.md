# Measuring the frame times / Frame-Zeiten messen

## English

Use this when the window feels slow while its size is changed.

1. Start the program with the variable set to a file that does not exist yet:
   `FS_FRAME_LOG=/tmp/x.csv fs-packages`
   (an existing file is never overwritten: the program says so on standard error and runs without a log).
2. For about 10 seconds drag a window corner back and forth, large and small. Then stop, and close the window.
3. Send us `/tmp/x.csv`. Please add: the program version (`fs-packages --version`), release or debug build, Wayland or X11, the screen scaling, and how many entries the catalog showed.

The file is a plain CSV, mode 0600, one line per frame:
`t_ms,kind,total_ms,check_ms,rebuild_ms,tick_ms,render_ms,chain_ms,width,height`

- `t_ms`: milliseconds since the program opened the log.
- `kind`: `resize` (first frame after the window size changed), `rebuild` (another frame that rebuilt the tree), `redraw`.
- `total_ms`: the whole frame. `check_ms`: does the tree need a rebuild. `rebuild_ms`: our build, tree, style and layout. `tick_ms`: animations. `render_ms`: painting on the GPU. `chain_ms`: what follows the paint. (Blinc does not time layout or the swapchain present on their own.)
- `width`, `height`: window size in logical pixels.

Without `FS_FRAME_LOG` the program does nothing extra. The same variable works in every program that opens its window with `fs-components-app`.

## Deutsch

Wenn sich das Fenster beim Groessenaendern traege anfuehlt.

1. Programm starten, die Variable zeigt auf eine Datei, die es noch nicht gibt:
   `FS_FRAME_LOG=/tmp/x.csv fs-packages`
   (eine vorhandene Datei wird nie ueberschrieben: das Programm sagt es auf der Fehlerausgabe und laeuft ohne Log).
2. Etwa 10 Sekunden lang eine Fensterecke hin und her ziehen, gross und klein. Dann aufhoeren und das Fenster schliessen.
3. Uns `/tmp/x.csv` geben. Bitte dazu: Programmversion (`fs-packages --version`), Release- oder Debug-Build, Wayland oder X11, Bildschirm-Skalierung, wie viele Eintraege der Katalog zeigte.

Die Datei ist ein einfaches CSV (Modus 0600), eine Zeile je Frame:
`t_ms,kind,total_ms,check_ms,rebuild_ms,tick_ms,render_ms,chain_ms,width,height`

- `t_ms`: Millisekunden seit dem Oeffnen des Logs.
- `kind`: `resize` (erster Frame nach Groessenaenderung), `rebuild` (anderer Frame mit Neuaufbau des Baums), `redraw`.
- `total_ms`: ganzer Frame. `check_ms`: braucht der Baum einen Neuaufbau. `rebuild_ms`: unser Aufbau, Baum, Stil, Layout. `tick_ms`: Animationen. `render_ms`: Malen auf der GPU. `chain_ms`: was nach dem Malen folgt. (Blinc misst Layout und Swapchain-Present nicht einzeln.)
- `width`, `height`: Fenstergroesse in logischen Pixeln.

Ohne `FS_FRAME_LOG` tut das Programm nichts zusaetzlich. Dieselbe Variable gilt in jedem Programm, dessen Fenster ueber `fs-components-app` laeuft.
