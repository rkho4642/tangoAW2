# Window
# Endonym for this locale; shown in the language picker.
LANGUAGE = English

window-title = tangoAW2
# Tooltip on the top bar's close button (fullscreen only).
window-quit = Exit tangoAW2
play-offline = Play offline
play-offline-tooltip = Play Advance Wars 2 on your own, no internet needed. Everything is unlocked; pick army colours on the Teams screen with SELECT.

# Crash handler dialogs (parent process)
crash = Oops, tangoAW2 has encountered an error and has crashed!

    When reporting this crash, please include the following log file:

    { $path }
crash-no-log = Oops, tangoAW2 has encountered an error and has crashed!

    { $error }

# Discord rich presence
discord-presence-looking = Looking for match
discord-presence-in-single-player = In single player
discord-presence-in-lobby = In lobby
discord-presence-in-progress = Match in progress

# Top-bar tabs
tab-play = Play
tab-replays = Replays
tab-settings = Settings

# Play selectors
play-no-game = No game selected
play-no-save = Select save

# Save management
save-actions = Save actions
save-open-folder = Open folder
save-duplicate = Duplicate
save-rename = Rename
save-delete = Delete
save-rename-confirm = Rename
save-delete-confirm = Delete
save-action-cancel = Cancel
save-delete-prompt = Delete { $name }?
save-name-placeholder = New name
save-new = New save
save-new-confirm = Create
save-template-default = (default)
save-template-pick = Pick a template…

# Empty-state hints
empty-scanning-title = Scanning your library…
empty-scanning-body = Reading ROMs and saves.
empty-no-roms-title = No game ROMs found
empty-no-roms-body = Put your Advance Wars 2: Black Hole Rising (USA) .gba file into:
empty-no-saves-title = No save files for this game
empty-no-saves-body = Drop a .sav for this game into:
# tangoAW2: a Dual Strike pack saved by an older version, its .nds gone.
ds-pack-outdated = Your Dual Strike pack was made by an older tangoAW2 and lacks this version's new sounds. To update it, put your Dual Strike .nds in the ROMs folder again.

# Play bottom strip
play-link-code = Link code (leave empty for a random one)
play-link-code-random = Random link code
play-play = Play
play-training = Training
training-pip = Opponent screen
training-opponent-view = Opponent view
opponent-view-off = Off
opponent-view-picture-in-picture = Picture-in-picture
opponent-view-stack-horizontally = Stack horizontally
opponent-view-stack-vertically = Stack vertically
training-swap = Switch sides
play-fight = Fight
play-cancel = Leave
play-status-idle = Enter a link code and press Fight to play online, or press Play offline to play alone.
play-status-connecting = Connecting to matchmaking server…
play-status-direct-connecting = Connecting to opponent…
play-status-waiting-opponent = Waiting for opponent…
play-status-negotiating = Negotiating…
play-status-failed = Connection failed: { $error }
play-status-peer-disconnected = The other player left.
play-status-signaling-version-too-old = This version of tangoAW2 is too old to play online. Please update tangoAW2.
play-status-signaling-version-too-new = The matchmaking server is out of date for this version of tangoAW2.
play-status-signaling-rejected = The matchmaking server refused the connection: { $reason }
play-status-signaling-unreachable = Couldn't reach the matchmaking server: { $error }
play-status-signaling-failed = Matchmaking failed: { $error }
play-status-peer-connection-failed = Couldn't connect to the other player: { $error }
play-status-negotiate-expected-hello = The other player didn't send the expected handshake.
play-status-negotiate-version-too-old = The other player is running an older version of tangoAW2.
play-status-negotiate-version-too-new = The other player is running a newer version of tangoAW2.
play-status-negotiate-failed = An error occurred during negotiation: { $error }
lobby-waiting = Waiting…
lobby-no-game = (no game selected)
lobby-latency = Ping: { $ms } ms
lobby-latency-direct = Ping (direct): { $ms } ms
lobby-latency-relayed = Ping (relayed): { $ms } ms
lobby-link-code = Link code: { $code }
lobby-direct-host = Hosting on UDP port: { $port }
lobby-direct-connect = Connecting via UDP: { $target }
lobby-handshake = Exchanging settings…
lobby-match-type = Armies
lobby-frame-delay-suggest = Suggest based on ping
lobby-no-match-types = (no match types for this game)
lobby-pick-game-first = Pick a game first

lobby-compat-ok = Compatible — ready to play.
lobby-compat-missing-game = One side hasn't picked a game.
lobby-compat-missing-rom = The other player's game isn't installed here.
lobby-compat-version-mismatch = Game versions don't match (different patch / ROM).
lobby-compat-sim-too-old = This game's netplay changed since your opponent's version of tangoAW2 — they need to update.
lobby-compat-sim-too-new = This game's netplay changed since your version of tangoAW2 — you need to update.
lobby-compat-match-mismatch = Match type doesn't match.
lobby-ready = Ready
lobby-unready = Unready
lobby-match-starting = Starting…
lobby-blind-peer-on = Opponent is hiding their setup.
lobby-blind-self-on = You are hiding your setup.
session-opponent = Opponent setup
session-self = My setup
session-back-to-session = Back to session
session-build-warning-title = Opponent has an invalid setup
session-build-warning-dismiss = Dismiss setup warning
session-build-warning-show-violations = View violations
session-build-warning-hide-violations = Hide violations
# PvP telemetry deck cell tooltips
session-stat-tps = Tick/s (current/max)
session-stat-skew = Skew
session-stat-lead = Lead
session-stat-depth = Misprediction depth
session-stat-ping = Network latency
# Post-match results screen
session-results-victory = Victory!
session-results-defeat = Defeat
session-results-draw = Draw
session-results-no-contest = Match ended
session-results-disconnected = Opponent disconnected
session-results-no-rounds = The match ended before a round was decided.
session-results-vs = vs { $nickname }
session-results-you = You
session-results-round = Round { $number }
session-results-draws = { $count ->
    [one] 1 round ended in a draw
   *[other] { $count } rounds ended in a draw
}
session-results-watch-replay = Watch replay
session-results-done = Done

save-copy = Copy
copied = Copied!

# Common
save-empty = This save has no data for this view.
play-no-selection = Select a game and a save to inspect.

# Replays
replays-filter-all-games = All games
replays-filter-any-time = Any time
replays-filter-past-day = Past 24 hours
replays-filter-past-week = Past week
replays-filter-past-month = Past month
replays-filter-past-year = Past year
replays-filter-search-placeholder = Search replays…
replays-analyzing = Analyzing replay…
replays-show-incomplete = Show incomplete
replays-direct-marker = (direct)
replays-watch = Watch
replays-watch-missing-rom = Watch (ROM for this game isn't scanned)
replays-export = Render
replays-export-progress = Rendering…
replays-export-cancel = Cancel
replays-export-cancelling = Cancelling…
replays-export-success = Render finished.
replays-export-error = Render failed: { $error }
replays-export-no-rounds = no rounds selected for export
replays-export-open = Open render
replays-export-reset = Reset
replays-export-scale = Scale
replays-export-scale-raw = raw
replays-export-disable-bgm = Mute music
replays-export-twosided = Two-sided
replays-export-rounds = Rounds:
replays-export-setup = Setup
replays-export-rounds-analyzing = Rounds: analyzing the match…
replays-export-save-as = Save as…
playback-close = Close
playback-play = Play
playback-pause = Pause
playback-options = Options
playback-speed = Speed
playback-speed-custom-screen = 2× during either custom screen
playback-input-display = Input display (I)
playback-pip = Opponent screen
playback-opponent-view = Opponent view
playback-swap-perspective = Opponent's perspective (Tab)
playback-clip-tools = Clip
playback-clip-start = Mark clip start
playback-clip-end = Mark clip end
playback-clip-clear = Clear clip marks
playback-clip-export = Export clip
playback-disconnect = Disconnect
playback-disconnect-prompt = Disconnect from this match?
playback-disconnect-detail = You will end the match with your opponent.
playback-cancel = Cancel
playback-reconnecting = Connection lost
playback-reconnecting-detail = Reconnecting…
playback-exit-hold = Quitting…
playback-exit-hold-detail = Keep holding Esc to quit — release to cancel.
playback-priming-match = Starting the match…
playback-priming-match-detail = Booting both games into their battle.
playback-priming-peer = Waiting for your opponent…
playback-priming-peer-detail = Their game is still starting up.
playback-priming-replay = Starting the replay…
playback-priming-replay-detail = Booting the games into their battle.
playback-priming-elapsed = { $secs }s
playback-priming-failed = The games didn't reach their battle.
replays-select-prompt = Select a replay.
replays-streamer-hidden = Match details hidden in streamer mode.
replays-streamer-show = Show
replays-queue-add = Add to queue
replays-queue-count = { $n } queued
replays-queue-play = Play queue
replays-queue-clear = Clear queue
replays-queue-remove = Remove from queue
replays-queue-missing = Replay file is gone
replays-queue-up-next = { $n } up next
replays-scanning = Scanning replays…
play-opponent = Opponent
replays-match-type = Armies:
replays-duration = Duration:
replays-round-count = { $count ->
    [one] 1 round
   *[other] { $count } rounds
}
replays-incomplete = incomplete
play-you = You

# Patches
patches-open-folder = Open folder

# Settings panel
settings-section-general = General
settings-section-graphics = Graphics
settings-section-netplay = Netplay
settings-section-audio = Audio
settings-volume = Volume
settings-disable-bgm-in-pvp = Mute music in netplay
settings-nickname = Nickname
settings-language = Language
settings-data-path = Data path
settings-streamer-mode = Streamer mode
settings-section-experimental = Experimental
settings-enable-save-editor = Enable save editor
settings-experimental-warning = Experimental features can break or corrupt your saves, may be changed or removed at any time, and may be missing checks that keep your saves legal for online play. Use them at your own risk.
settings-section-about = About
settings-section-input = Input
settings-input-press-key = Press a key or button…
settings-input-add = Add binding
settings-input-reset = Reset to defaults
settings-input-select-hint = Click a button to edit its bindings
input-key-up = Up
input-key-down = Down
input-key-left = Left
input-key-right = Right
input-key-a = A
input-key-b = B
input-key-l = L
input-key-r = R
input-key-start = Start
input-key-select = Select
input-key-speed-up = Fast-forward
input-gamepad-south = Button A
input-gamepad-east = Button B
input-gamepad-west = Button X
input-gamepad-north = Button Y
input-gamepad-select = Select
input-gamepad-start = Start
input-gamepad-mode = Guide
input-gamepad-left-thumb = Left Stick
input-gamepad-right-thumb = Right Stick
input-gamepad-left-shoulder = LB
input-gamepad-right-shoulder = RB
input-gamepad-dpad-up = D-Pad Up
input-gamepad-dpad-down = D-Pad Down
input-gamepad-dpad-left = D-Pad Left
input-gamepad-dpad-right = D-Pad Right
input-gamepad-misc1 = Misc 1
input-gamepad-misc2 = Misc 2
input-gamepad-misc3 = Misc 3
input-gamepad-misc4 = Misc 4
input-gamepad-misc5 = Misc 5
input-gamepad-misc6 = Misc 6
input-gamepad-right-paddle1 = Right Paddle 1
input-gamepad-left-paddle1 = Left Paddle 1
input-gamepad-right-paddle2 = Right Paddle 2
input-gamepad-left-paddle2 = Left Paddle 2
input-gamepad-touchpad = Touchpad
input-gamepad-axis-left-stick-x = Left Stick X
input-gamepad-axis-left-stick-y = Left Stick Y
input-gamepad-axis-right-stick-x = Right Stick X
input-gamepad-axis-right-stick-y = Right Stick Y
input-gamepad-axis-trigger-left = Left Trigger
input-gamepad-axis-trigger-right = Right Trigger
settings-theme = Theme
settings-theme-advance-wars = Advance Wars
settings-background-image = Background image
settings-background-image-none = Drawn field
settings-background-image-choose = Choose…
settings-background-image-clear = Clear
settings-theme-dark = Dark
settings-theme-light = Light
settings-accent = Accent color
settings-accent-green = Green
settings-accent-blue = Blue
settings-accent-red = Red
settings-accent-pink = Pink
settings-accent-yellow = Yellow
settings-accent-purple = Purple
settings-group-profile = Profile
settings-group-interface = Interface
settings-group-storage = Storage
settings-group-updates = Updates
settings-group-window = Window
settings-group-emulator = Emulator
settings-matchmaking-endpoint = Matchmaking endpoint
settings-data-folder = Data folder
settings-data-folder-change = Change…
settings-data-folder-ios = On My iPhone or iPad › tangoAW2
settings-data-folder-open-ios = Show in Files
settings-enable-updater = Automatically check for app updates
settings-allow-prerelease-upgrades = Include prereleases when checking for app updates
settings-netplay-frame-delay = Frame delay
settings-use-relay = Use relay server
settings-use-relay-auto = Auto
settings-use-relay-always = Always
settings-use-relay-never = Never
settings-window-size = Window size
settings-fullscreen = Fullscreen
settings-ui-scale = UI scale
settings-video-filter = Video filter
settings-fractional-scaling = Fractional scaling
updater-current-version = Current version: { $version }
updater-latest-version = Latest version: { $version }
updater-loading = checking…
updater-up-to-date = v{ $version } (up to date)
updater-downloading = Downloading: { $pct }%
updater-ready-to-update = Update downloaded and ready to install.
updater-update-now = Update now

# Welcome screen
welcome-title = Welcome to tangoAW2!
welcome-subtitle = There's just a few steps you'll need to complete before you can start playing.
welcome-continue = Continue
welcome-step-roms = Add your Advance Wars 2 ROM
welcome-step-roms-description = Put your Advance Wars 2: Black Hole Rising (USA) .gba file into:
welcome-step-roms-detected = { $count } ROMs detected.
welcome-step-nickname = Set your nickname
welcome-step-nickname-description = You can change this at any time in Settings.
welcome-open-folder = Open ROMs folder
# iPhone / iPad: the ROMs come in through the Files picker, or by copying
# them into the app's folder in the Files app.
welcome-step-roms-description-ios = Import your Advance Wars 2: Black Hole Rising (USA) .gba file (and, for the Dual Strike features, your Advance Wars: Dual Strike (USA) .nds), or copy them in the Files app to:
welcome-roms-folder-ios = On My iPhone or iPad › tangoAW2 › roms
welcome-import-roms = Import ROMs…
welcome-roms-needed = Add at least one ROM before continuing.

# Common actions
rescan = Rescan

# Game names live in games.ftl — same Fluent attribute scheme the
# legacy app uses (game-<family> = base name; .variant-N for each
# regional/colour variant; .match-type-X-Y for per-mode labels).

input-key-mic = Blow into mic
input-key-x = X
input-key-y = Y
settings-ds-primary-screen = Primary screen
settings-ds-primary-screen-touch = Touch screen
settings-ds-primary-screen-upper = Upper screen
settings-ds-screen-stacking = Stacking
settings-ds-screen-stacking-horizontal = Horizontal
settings-ds-screen-stacking-primary-only = Primary screen only
settings-ds-screen-stacking-vertical = Vertical
settings-group-ds = Nintendo DS
