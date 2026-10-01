// UIKit, GameController and AVFoundation glue for the iOS build of
// tangoAW2. Compiled by build.rs for iOS targets only; the Rust side is
// platform/ios/mod.rs. Everything here is plain C entry points so the
// Rust side needs no Objective-C runtime crate.

#import <AVFoundation/AVFoundation.h>
#import <GameController/GameController.h>
#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

// ---------------------------------------------------------------------------
// Process setup

static double g_keyboard_height;

// Height in points of the on-screen keyboard over the app (0 when hidden).
double tango_ios_keyboard_height(void) { return g_keyboard_height; }

void tango_ios_init(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        // A match is long stretches of thinking without touching the
        // screen; the screen must not lock under a netplay game.
        UIApplication.sharedApplication.idleTimerDisabled = YES;
    });

    // The on-screen keyboard's height, so the UI can lift what is being
    // typed into above it.
    [NSNotificationCenter.defaultCenter
        addObserverForName:UIKeyboardWillChangeFrameNotification
                    object:nil
                     queue:NSOperationQueue.mainQueue
                usingBlock:^(NSNotification *note) {
                    CGRect end = [note.userInfo[UIKeyboardFrameEndUserInfoKey] CGRectValue];
                    CGFloat screen_h = UIScreen.mainScreen.bounds.size.height;
                    g_keyboard_height = MAX(0, screen_h - end.origin.y);
                }];
    [NSNotificationCenter.defaultCenter addObserverForName:UIKeyboardWillHideNotification
                                                    object:nil
                                                     queue:NSOperationQueue.mainQueue
                                                usingBlock:^(NSNotification *note) {
                                                    g_keyboard_height = 0;
                                                }];

    // Testing in the Simulator, which cannot be rotated from the command
    // line: TANGOAW2_ORIENTATION=landscape|portrait.
    const char *orient = getenv("TANGOAW2_ORIENTATION");
    if (orient) {
        BOOL landscape = strcmp(orient, "landscape") == 0;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 1500 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
            if (@available(iOS 16.0, *)) {
                for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
                    if (![scene isKindOfClass:UIWindowScene.class]) continue;
                    UIWindowSceneGeometryPreferencesIOS *prefs = [[UIWindowSceneGeometryPreferencesIOS alloc]
                        initWithInterfaceOrientations:landscape ? UIInterfaceOrientationMaskLandscapeRight
                                                                : UIInterfaceOrientationMaskPortrait];
                    [(UIWindowScene *)scene requestGeometryUpdateWithPreferences:prefs
                                                                    errorHandler:^(NSError *e) {
                                                                        NSLog(@"tangoAW2: rotate: %@", e);
                                                                    }];
                }
            }
        });
    }

    // Game audio: plays with the silent switch on and over other audio
    // being stopped, like a console. A short IO buffer keeps latency low.
    AVAudioSession *session = AVAudioSession.sharedInstance;
    NSError *err = nil;
    if (![session setCategory:AVAudioSessionCategoryPlayback
                         mode:AVAudioSessionModeDefault
                      options:0
                        error:&err]) {
        NSLog(@"tangoAW2: AVAudioSession setCategory: %@", err);
    }
    [session setPreferredIOBufferDuration:0.005 error:nil];
    if (![session setActive:YES error:&err]) {
        NSLog(@"tangoAW2: AVAudioSession setActive: %@", err);
    }
}

static UIWindow *key_window(void) {
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if (![scene isKindOfClass:UIWindowScene.class]) continue;
        UIWindowScene *ws = (UIWindowScene *)scene;
        for (UIWindow *w in ws.windows) {
            if (w.isKeyWindow) return w;
        }
        if (ws.windows.count > 0) return ws.windows.firstObject;
    }
    return nil;
}

// Safe-area insets of the app's window in points: top, left, bottom, right.
// Callable from any thread; reads on the main thread.
void tango_ios_safe_area(double *out) {
    __block UIEdgeInsets insets = UIEdgeInsetsZero;
    void (^read)(void) = ^{
        UIWindow *w = key_window();
        if (w) insets = w.safeAreaInsets;
    };
    if (NSThread.isMainThread) {
        read();
    } else {
        dispatch_sync(dispatch_get_main_queue(), read);
    }
    out[0] = insets.top;
    out[1] = insets.left;
    out[2] = insets.bottom;
    out[3] = insets.right;
}


void tango_ios_open_url(const char *url) {
    NSString *s = [NSString stringWithUTF8String:url];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSURL *u = [NSURL URLWithString:s];
        if (u) [UIApplication.sharedApplication openURL:u options:@{} completionHandler:nil];
    });
}

void tango_ios_set_clipboard_text(const char *text) {
    NSString *s = [NSString stringWithUTF8String:text];
    dispatch_async(dispatch_get_main_queue(), ^{
        UIPasteboard.generalPasteboard.string = s;
    });
}

void tango_ios_set_clipboard_png(const unsigned char *bytes, size_t len) {
    NSData *data = [NSData dataWithBytes:bytes length:len];
    dispatch_async(dispatch_get_main_queue(), ^{
        [UIPasteboard.generalPasteboard setData:data forPasteboardType:UTTypePNG.identifier];
    });
}

// ---------------------------------------------------------------------------
// Document picker: import copies of files the user picks in Files.

typedef void (*tango_pick_cb)(void *ctx, const char *const *paths, size_t n);

@interface TangoPickerDelegate : NSObject <UIDocumentPickerDelegate>
@property(nonatomic) tango_pick_cb cb;
@property(nonatomic) void *ctx;
@end

static TangoPickerDelegate *g_picker_delegate;

@implementation TangoPickerDelegate
- (void)finish:(NSArray<NSURL *> *)urls {
    size_t n = urls.count;
    const char **paths = calloc(n ? n : 1, sizeof(char *));
    for (size_t i = 0; i < n; i++) paths[i] = urls[i].fileSystemRepresentation;
    tango_pick_cb cb = self.cb;
    void *ctx = self.ctx;
    self.cb = NULL;
    if (cb) cb(ctx, paths, n);
    free(paths);
    g_picker_delegate = nil;
}
- (void)documentPicker:(UIDocumentPickerViewController *)controller
    didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls {
    [self finish:urls];
}
- (void)documentPickerWasCancelled:(UIDocumentPickerViewController *)controller {
    [self finish:@[]];
}
@end

// kind 0: any file (ROMs, packs). kind 1: images.
// `cb` is called exactly once, on the main thread, with the picked files'
// temporary copies (n = 0 when cancelled or the picker could not open).
void tango_ios_pick_files(int kind, int multiple, tango_pick_cb cb, void *ctx) {
    dispatch_async(dispatch_get_main_queue(), ^{
        UIWindow *w = key_window();
        UIViewController *vc = w.rootViewController;
        while (vc.presentedViewController) vc = vc.presentedViewController;
        if (!vc || g_picker_delegate) {
            cb(ctx, NULL, 0);
            return;
        }
        NSArray<UTType *> *types = kind == 1 ? @[ UTTypeImage ] : @[ UTTypeData, UTTypeItem ];
        UIDocumentPickerViewController *picker =
            [[UIDocumentPickerViewController alloc] initForOpeningContentTypes:types asCopy:YES];
        picker.allowsMultipleSelection = multiple != 0;
        TangoPickerDelegate *d = [TangoPickerDelegate new];
        d.cb = cb;
        d.ctx = ctx;
        g_picker_delegate = d;
        picker.delegate = d;
        [vc presentViewController:picker animated:YES completion:nil];
    });
}

// ---------------------------------------------------------------------------
// Controllers: MFi / Xbox / PlayStation pads through GameController.

typedef struct {
    uint32_t id;
    // Bits, in this order: A(south) B(east) X(west) Y(north) options(select)
    // menu(start) home LS RS L1 R1 up down left right
    uint32_t buttons;
    // left x, left y, right x, right y (y down-positive), L2, R2
    float axes[6];
} TangoPad;

static NSMapTable<GCController *, NSNumber *> *g_pad_ids;
static uint32_t g_next_pad_id = 1;

int tango_ios_pads(TangoPad *out, int max) {
    if (!g_pad_ids) g_pad_ids = [NSMapTable weakToStrongObjectsMapTable];
    int n = 0;
    for (GCController *c in GCController.controllers) {
        GCExtendedGamepad *g = c.extendedGamepad;
        if (!g || n >= max) continue;
        NSNumber *idn = [g_pad_ids objectForKey:c];
        if (!idn) {
            idn = @(g_next_pad_id++);
            [g_pad_ids setObject:idn forKey:c];
        }
        TangoPad *p = &out[n++];
        p->id = idn.unsignedIntValue;
        uint32_t b = 0;
        int i = 0;
#define BIT(x) do { if ((x)) b |= 1u << i; i++; } while (0)
        BIT(g.buttonA.isPressed);
        BIT(g.buttonB.isPressed);
        BIT(g.buttonX.isPressed);
        BIT(g.buttonY.isPressed);
        BIT(g.buttonOptions.isPressed);
        BIT(g.buttonMenu.isPressed);
        BIT(g.buttonHome.isPressed);
        BIT(g.leftThumbstickButton.isPressed);
        BIT(g.rightThumbstickButton.isPressed);
        BIT(g.leftShoulder.isPressed);
        BIT(g.rightShoulder.isPressed);
        BIT(g.dpad.up.isPressed);
        BIT(g.dpad.down.isPressed);
        BIT(g.dpad.left.isPressed);
        BIT(g.dpad.right.isPressed);
#undef BIT
        p->buttons = b;
        p->axes[0] = g.leftThumbstick.xAxis.value;
        p->axes[1] = -g.leftThumbstick.yAxis.value;
        p->axes[2] = g.rightThumbstick.xAxis.value;
        p->axes[3] = -g.rightThumbstick.yAxis.value;
        p->axes[4] = g.leftTrigger.value;
        p->axes[5] = g.rightTrigger.value;
    }
    return n;
}

// Hardware keyboard (iPad Magic Keyboard, Bluetooth keyboards): whether
// each of `n` HID usage codes is held.
void tango_ios_keys(const uint16_t *codes, uint8_t *held, int n) {
    GCKeyboardInput *k = GCKeyboard.coalescedKeyboard.keyboardInput;
    for (int i = 0; i < n; i++) {
        held[i] = k ? [k buttonForKeyCode:(GCKeyCode)codes[i]].isPressed : 0;
    }
}
