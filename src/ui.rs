use std::cell::Cell;

use objc2::rc::{Allocated, Retained};
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold,
    NSImage, NSTextAlignment, NSTextField, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

pub struct ProgressBarIvars {
    percentage: Cell<u8>,
    is_charging: Cell<bool>,
}

declare_class!(
    pub struct BatteryProgressBarView;

    unsafe impl ClassType for BatteryProgressBarView {
        #[inherits(objc2_app_kit::NSResponder, objc2_foundation::NSObject)]
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "BatteryBarCapsuleProgressView";
    }

    impl DeclaredClass for BatteryProgressBarView {
        type Ivars = ProgressBarIvars;
    }

    unsafe impl BatteryProgressBarView {
        #[method_id(initWithFrame:)]
        fn init_with_frame(this: Allocated<Self>, frame: NSRect) -> Option<Retained<Self>> {
            let this = this.set_ivars(ProgressBarIvars {
                percentage: Cell::new(0),
                is_charging: Cell::new(false),
            });
            unsafe { msg_send_id![super(this), initWithFrame: frame] }
        }

        #[method(drawRect:)]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            let bounds = self.bounds();
            let radius = bounds.size.height / 2.0;

            unsafe {
                // 1. Capsule Track Background (quaternaryLabelColor adapts to Dark/Light mode)
                let track_color = NSColor::quaternaryLabelColor();
                track_color.setFill();
                let track_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    bounds, radius, radius,
                );
                track_path.fill();

                // 2. Capsule Fill
                let pct = self.ivars().percentage.get().min(100);
                let charging = self.ivars().is_charging.get();

                if pct > 0 {
                    let fill_color = if charging || pct > 20 {
                        NSColor::systemGreenColor()
                    } else if pct > 10 {
                        NSColor::systemOrangeColor()
                    } else {
                        NSColor::systemRedColor()
                    };
                    fill_color.setFill();

                    let raw_width = bounds.size.width * (pct as f64 / 100.0);
                    let fill_width = raw_width.max(bounds.size.height).min(bounds.size.width);
                    let fill_rect = NSRect::new(
                        NSPoint::new(bounds.origin.x, bounds.origin.y),
                        NSSize::new(fill_width, bounds.size.height),
                    );
                    let fill_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                        fill_rect, radius, radius,
                    );
                    fill_path.fill();
                }
            }
        }
    }
);

impl BatteryProgressBarView {
    pub fn new(frame: NSRect, mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send_id![mtm.alloc::<Self>(), initWithFrame: frame] }
    }

    pub fn update(&self, percentage: u8, is_charging: bool) {
        self.ivars().percentage.set(percentage);
        self.ivars().is_charging.set(is_charging);
        unsafe { self.setNeedsDisplay(true) };
    }
}

/// Native Control Center–style Battery Header Card embedded at the top of NSMenu
pub struct HeaderCardView {
    pub container: Retained<NSView>,
    pub title_label: Retained<NSTextField>,
    pub percentage_label: Retained<NSTextField>,
    pub progress_bar: Retained<BatteryProgressBarView>,
    pub status_label: Retained<NSTextField>,
    pub source_label: Retained<NSTextField>,
}

impl HeaderCardView {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let width = 276.0;
        let height = 96.0;
        let pad_x = 14.0;
        let inner_w = width - (pad_x * 2.0);

        let container_frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        let container = unsafe { NSView::initWithFrame(mtm.alloc(), container_frame) };

        unsafe {
            // 1. Left Header Title ("Battery")
            let title_label = NSTextField::labelWithString(&NSString::from_str("Battery"), mtm);
            title_label.setFrame(NSRect::new(
                NSPoint::new(pad_x, 68.0),
                NSSize::new(165.0, 20.0),
            ));
            let bold_font = NSFont::boldSystemFontOfSize(13.5);
            title_label.setFont(Some(&bold_font));
            let primary_color = NSColor::labelColor();
            title_label.setTextColor(Some(&primary_color));
            container.addSubview(&title_label);

            // 2. Right Percentage Label ("76%")
            let percentage_label = NSTextField::labelWithString(&NSString::from_str("--%"), mtm);
            percentage_label.setFrame(NSRect::new(
                NSPoint::new(width - pad_x - 80.0, 68.0),
                NSSize::new(80.0, 20.0),
            ));
            let pct_font =
                NSFont::monospacedDigitSystemFontOfSize_weight(13.5, NSFontWeightSemibold);
            percentage_label.setFont(Some(&pct_font));
            percentage_label.setTextColor(Some(&primary_color));
            percentage_label.setAlignment(NSTextAlignment::Right);
            container.addSubview(&percentage_label);

            // 3. Capsule Progress Bar
            let bar_frame = NSRect::new(NSPoint::new(pad_x, 51.0), NSSize::new(inner_w, 8.0));
            let progress_bar = BatteryProgressBarView::new(bar_frame, mtm);
            container.addSubview(&progress_bar);

            // 4. Primary Status Subtitle ("About 3h 15m remaining")
            let status_label =
                NSTextField::labelWithString(&NSString::from_str("Calculating…"), mtm);
            status_label.setFrame(NSRect::new(
                NSPoint::new(pad_x, 26.0),
                NSSize::new(inner_w, 17.0),
            ));
            let sub_font_1 = NSFont::systemFontOfSize_weight(12.0, NSFontWeightMedium);
            status_label.setFont(Some(&sub_font_1));
            let secondary_color = NSColor::secondaryLabelColor();
            status_label.setTextColor(Some(&secondary_color));
            container.addSubview(&status_label);

            // 5. Secondary Power Source Subtitle ("Power Source: Battery")
            let source_label =
                NSTextField::labelWithString(&NSString::from_str("Power Source: --"), mtm);
            source_label.setFrame(NSRect::new(
                NSPoint::new(pad_x, 8.0),
                NSSize::new(inner_w, 16.0),
            ));
            let sub_font_2 = NSFont::systemFontOfSize_weight(11.5, NSFontWeightRegular);
            status_label.setFont(Some(&sub_font_1));
            source_label.setFont(Some(&sub_font_2));
            source_label.setTextColor(Some(&secondary_color));
            container.addSubview(&source_label);

            Self {
                container,
                title_label,
                percentage_label,
                progress_bar,
                status_label,
                source_label,
            }
        }
    }
}

/// Loads an Apple SF Symbol template image with an optional fallback symbol name
pub fn load_sf_symbol(primary: &str, fallback: Option<&str>) -> Option<Retained<NSImage>> {
    unsafe {
        let name_ns = NSString::from_str(primary);
        let img = NSImage::imageWithSystemSymbolName_accessibilityDescription(&name_ns, None)
            .or_else(|| {
                let fb = fallback?;
                let fb_ns = NSString::from_str(fb);
                NSImage::imageWithSystemSymbolName_accessibilityDescription(&fb_ns, None)
            })?;
        img.setTemplate(true);
        Some(img)
    }
}
