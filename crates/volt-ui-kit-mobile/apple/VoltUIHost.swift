// UIKit hosts the layer, keyboard and accessibility proxies, not the screens.
import UIKit
import Metal
import QuartzCore
import CoreText

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {}
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = VoltRootController()
        // Debugging aid: VOLT_ANIMATION_SPEED=0.1 plays UIKit animations ten
        // times slower, to inspect transitions frame by frame.
        if let speed = ProcessInfo.processInfo.environment["VOLT_ANIMATION_SPEED"].flatMap(Float.init), speed > 0 {
            window.layer.speed = speed
        }
        window.makeKeyAndVisible()
        self.window = window
    }
    func sceneWillResignActive(_ scene: UIScene) { (window?.rootViewController as? VoltRootController)?.canvas.suspend(true) }
    func sceneDidBecomeActive(_ scene: UIScene) { (window?.rootViewController as? VoltRootController)?.canvas.suspend(false) }
}
/// One page of a Volt navigation stack. The top page holds the live canvas;
/// pages below keep a snapshot of how they looked, for back swipes.
final class VoltPageController: UIViewController {
    let depth: Int
    fileprivate var bar: NavInfo?
    private var snapshot: UIView?
    init(depth: Int) { self.depth = depth; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError("Use programmatic host") }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemGroupedBackground
    }
    /// Pin the live canvas above the keyboard. Full bleed runs it under the
    /// bars and the home indicator, where its main scroll view keeps insets;
    /// otherwise it sits below the bar.
    func attach(_ canvas: VoltCanvas, fullBleed: Bool) {
        loadViewIfNeeded()
        snapshot?.removeFromSuperview()
        snapshot = nil
        canvas.removeFromSuperview()
        view.addSubview(canvas)
        canvas.translatesAutoresizingMaskIntoConstraints = false
        // Full bleed keeps its size when the keyboard opens: the page's scroll
        // view takes the keyboard as an inset. Otherwise it sits above it.
        NSLayoutConstraint.activate([
            canvas.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            canvas.topAnchor.constraint(equalTo: fullBleed ? view.topAnchor : view.safeAreaLayoutGuide.topAnchor),
            canvas.bottomAnchor.constraint(equalTo: fullBleed ? view.bottomAnchor : view.keyboardLayoutGuide.topAnchor)
        ])
    }
    /// Show how the page looked when the canvas left it.
    func keep(_ image: UIView?, frame: CGRect) {
        snapshot?.removeFromSuperview()
        snapshot = image
        guard let image else { return }
        image.frame = frame
        view.addSubview(image)
    }
}
/// UIKit's navigation controller around Volt's pages: the real bar, back
/// button, swipe-back and push animations. Rust's history decides the pages.
/// The app's root. When Rust describes a tab view, UIKit's own tab bar (Liquid
/// Glass on iOS 26) switches between sections, each with its own navigation
/// stack, and the one Volt canvas moves to the section on screen. Without a
/// tab view there's a single stack and no bar. Rust decides the selection:
/// a tap asks Rust, and the bar follows what Rust then reports.
final class VoltRootController: UITabBarController, UITabBarControllerDelegate {
    let canvas = VoltCanvas()
    private var stacks: [VoltNavigationController] = []
    private var tabIds: [String] = []
    private var covered = false
    init() {
        super.init(nibName: nil, bundle: nil)
        delegate = self
        let first = VoltNavigationController(canvas: canvas)
        stacks = [first]
        setViewControllers(stacks, animated: false)
        canvas.navigation = first
        canvas.root = self
        first.adopt(nil)
        showTabBar(false)
    }
    required init?(coder: NSCoder) { fatalError("Use programmatic host") }
    private func showTabBar(_ shown: Bool) {
        if #available(iOS 18.0, *) {
            if isTabBarHidden == shown { setTabBarHidden(!shown, animated: false) }
        } else if tabBar.isHidden == shown {
            tabBar.isHidden = !shown
        }
    }

    /// Match UIKit's tabs to Rust's. `before` is how the canvas looked
    /// before the event, kept by the section that is leaving.
    fileprivate func sync(_ tabs: TabsInfo?, before: UIView?, accent: UIColor, modal: Bool) {
        guard let tabs, !tabs.items.isEmpty else { showTabBar(false); return }
        // A modal sheet or dialog covers the tab bar, as on iOS.
        if modal != covered {
            covered = modal
            // Fade it rather than hide it: hiding the tab bar changes the
            // page's layout, and the page behind a sheet must not move.
            tabBar.isUserInteractionEnabled = !modal
            UIView.animate(withDuration: 0.25) { self.tabBar.alpha = modal ? 0 : 1 }
        }
        if modal { return }
        if tabBar.tintColor != accent { tabBar.tintColor = accent }
        tabIds = tabs.items.map(\.id)
        while stacks.count < tabs.items.count { stacks.append(VoltNavigationController(canvas: canvas)) }
        for (stack, item) in zip(stacks, tabs.items) {
            let bar = stack.tabBarItem!
            if bar.title != item.label { bar.title = item.label }
            let name = item.symbol ?? systemSymbol(item.icon)
            if bar.accessibilityIdentifier != item.id || stack.symbol != name {
                bar.image = UIImage(systemName: name) ?? UIImage(systemName: "circle.fill")
                bar.accessibilityIdentifier = item.id
                stack.symbol = name
            }
            if bar.badgeValue != item.badge { bar.badgeValue = item.badge }
        }
        let target = stacks[min(tabs.selected, stacks.count - 1)]
        if canvas.navigation !== target {
            canvas.navigation?.leave(before ?? canvas.snapshotView(afterScreenUpdates: false))
            canvas.navigation = target
            target.adopt(nil)
        }
        if stacks.count > tabs.items.count { stacks.removeLast(stacks.count - tabs.items.count) }
        if viewControllers?.count != stacks.count || !zip(viewControllers ?? [], stacks).allSatisfy({ $0 === $1 }) {
            setViewControllers(stacks, animated: false)
        }
        if selectedViewController !== target { selectedViewController = target }
        showTabBar(true)
    }
    /// A tap on a tab: Rust selects it (or reselects the open one) and the
    /// bar follows on the next snapshot.
    func tabBarController(_ controller: UITabBarController, shouldSelect viewController: UIViewController) -> Bool {
        if let i = stacks.firstIndex(where: { $0 === viewController }), i < tabIds.count {
            canvas.navigate(tabIds[i])
        }
        return false
    }
}
/// SF Symbols for Volt's icons, for tabs that don't name their own.
private func systemSymbol(_ icon: String) -> String {
    [
        "Home": "house.fill", "Settings": "gearshape.fill", "User": "person.fill",
        "Users": "person.2.fill", "Bell": "bell.fill", "Mail": "envelope.fill",
        "Calendar": "calendar", "Clock": "clock.fill", "Star": "star.fill", "StarFilled": "star.fill",
        "Heart": "heart.fill", "Bookmark": "bookmark.fill", "Inbox": "tray.fill",
        "Folder": "folder.fill", "Search": "magnifyingglass", "Grid": "square.grid.2x2.fill",
        "List": "list.bullet", "Globe": "globe", "Bolt": "bolt.fill", "Image": "photo.fill",
        "File": "doc.fill", "FileText": "doc.text.fill", "Send": "paperplane.fill",
        "Check": "checkmark.circle.fill", "Command": "command", "Play": "play.fill",
        "Lock": "lock.fill", "Palette": "paintpalette.fill", "Sliders": "slider.horizontal.3",
    ][icon] ?? "circle.fill"
}
final class VoltNavigationController: UINavigationController, UINavigationControllerDelegate {
    unowned let canvas: VoltCanvas
    /// The symbol on this section's tab.
    fileprivate var symbol: String?
    /// The depth Rust last reported.
    private var depth = 0
    /// The stack's Back control; the same ID on every page.
    private var backId: String?
    /// The canvas runs under the bars (its page has a main scroll view).
    /// Pages start that way, since most scroll; one without switches back.
    private var fullBleed = true
    /// Run the canvas under the bars when the page has a main scroll view,
    /// and give UIKit that scroll view for large titles and edge effects.
    fileprivate func setContent(_ scroll: UIScrollView?) {
        let bleed = scroll != nil
        if bleed != fullBleed, let top = pages.last {
            fullBleed = bleed
            top.attach(canvas, fullBleed: bleed)
        }
        pages.last?.setContentScrollView(scroll, for: .top)
        pages.last?.setContentScrollView(scroll, for: .bottom)
    }
    init(canvas: VoltCanvas) {
        self.canvas = canvas
        let root = VoltPageController(depth: 0)
        super.init(rootViewController: root)
        delegate = self
        navigationBar.prefersLargeTitles = true
        setNavigationBarHidden(true, animated: false)
    }
    /// The canvas arrives: this section is on screen.
    fileprivate func adopt(_: UIView?) {
        pages.last?.attach(canvas, fullBleed: fullBleed)
    }
    /// The canvas leaves for another section; keep how this one looked.
    fileprivate func leave(_ image: UIView?) {
        pages.last?.keep(image, frame: canvas.frame)
    }
    required init?(coder: NSCoder) { fatalError("Use programmatic host") }
    private var pages: [VoltPageController] { viewControllers.compactMap { $0 as? VoltPageController } }
    /// Match UIKit's stack to Rust's. `before` is the canvas as it looked
    /// before the event, for the page that is leaving.
    fileprivate func sync(_ bar: NavInfo?, before: UIView?, background: UIColor, accent: UIColor) {
        pages.forEach { $0.view.backgroundColor = background }
        // The bar's buttons and back chevron follow the page's accent.
        if navigationBar.tintColor != accent { navigationBar.tintColor = accent }
        guard let bar else {
            if !isNavigationBarHidden { setNavigationBarHidden(true, animated: false) }
            return
        }
        if isNavigationBarHidden { setNavigationBarHidden(false, animated: false) }
        if let back = bar.back { backId = back }
        let frame = canvas.frame
        var stack = pages
        if bar.depth > depth, let top = stack.last {
            // Pushed: the page underneath keeps its last look.
            top.keep(before ?? canvas.snapshotView(afterScreenUpdates: false), frame: frame)
            var added: [VoltPageController] = []
            for d in (depth + 1)...bar.depth { added.append(VoltPageController(depth: d)) }
            configure(added.last!, bar)
            added.last!.attach(canvas, fullBleed: fullBleed)
            stack.append(contentsOf: added)
            depth = bar.depth
            setViewControllers(stack, animated: true)
        } else if bar.depth < depth {
            // Rust went back: the leaving page shows its last look.
            stack.last?.keep(before ?? canvas.snapshotView(afterScreenUpdates: false), frame: frame)
            let target = stack[min(bar.depth, stack.count - 1)]
            configure(target, bar)
            target.attach(canvas, fullBleed: fullBleed)
            depth = bar.depth
            popToViewController(target, animated: true)
        } else if let top = stack.last {
            configure(top, bar)
        }
    }
    private func configure(_ page: VoltPageController, _ bar: NavInfo) {
        guard page.bar != bar else { return }
        page.bar = bar
        let item = page.navigationItem
        item.title = bar.title
        item.largeTitleDisplayMode = bar.largeTitle ? .always : .never
        item.hidesBackButton = bar.back == nil
        if page === topViewController || page.depth == bar.depth {
            setBackGestures(bar.back != nil)
        }
        // rightBarButtonItems run from the trailing edge inward.
        item.rightBarButtonItems = bar.buttons.reversed().map { button in
            UIBarButtonItem(title: button.label, primaryAction: UIAction { [weak self] _ in
                self?.canvas.navigate(button.id)
            })
        }
    }
    private var covered = false
    /// A modal is open over the page: the bar's controls dim and stop
    /// taking taps until it closes.
    fileprivate func cover(_ modal: Bool) {
        guard modal != covered else { return }
        covered = modal
        navigationBar.isUserInteractionEnabled = !modal
        navigationBar.tintAdjustmentMode = modal ? .dimmed : .automatic
        // A modal covers the bar: it fades out rather than float above the
        // scrim, and comes back when the modal closes.
        UIView.animate(withDuration: 0.25) { self.navigationBar.alpha = modal ? 0 : 1 }
        setBackGestures(!modal && (pages.last?.bar?.back != nil))
    }
    private var backEnabled = true
    private var held = false
    private var heldContent = false
    private func setBackGestures(_ enabled: Bool) {
        backEnabled = enabled
        applyBackGestures()
    }
    private func applyBackGestures() {
        interactivePopGestureRecognizer?.isEnabled = backEnabled && !held
        if #available(iOS 26.0, *) {
            interactiveContentPopGestureRecognizer?.isEnabled = backEnabled && !held && !heldContent
        }
    }
    /// Pause back swipes while a finger drags a control sideways.
    fileprivate func holdBackGestures(_ hold: Bool) {
        held = hold
        applyBackGestures()
    }
    /// Pause only the swipe-from-anywhere back gesture, for a touch on a
    /// sideways carousel; the screen-edge swipe still goes back.
    fileprivate func holdContentBackGesture(_ hold: Bool) {
        heldContent = hold
        applyBackGestures()
    }
    /// UIKit went back (back button or swipe): bring the canvas and Rust along.
    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        guard canvas.navigation === self, let shown = viewController as? VoltPageController, shown.depth < depth else { return }
        shown.attach(canvas, fullBleed: fullBleed)
        let steps = depth - shown.depth
        depth = shown.depth
        guard let back = backId else { return }
        for _ in 0..<steps { canvas.navigate(back) }
    }
}
private struct Node: Decodable {
    let id: String
    let label: String
    let value: String?
    let enabled: Bool
    let kind: String
    let password: Bool
    let multiline: Bool?
    let readOnly: Bool
    let bounds: [CGFloat]
    let visibleBounds: [CGFloat]
    let checked: Bool?
    /// A slider's value, minimum, maximum and step.
    let numeric: [CGFloat]?
    /// Actions besides activating it (a row's swipe actions, its menu).
    let actions: [NodeAction]?
    /// A text input wants the Done bar above the keyboard.
    let doneBar: Bool?
    /// What an empty field says.
    let placeholder: String?
    /// Pressing it while typing leaves the keyboard up.
    let keepsKeyboard: Bool?
    /// The colour behind a field and the corner radius of what paints it.
    let backdrop: Backdrop?
    var visibleRect: CGRect { visibleBounds.count == 4 && visibleBounds[2] > 0 && visibleBounds[3] > 0 ? CGRect(x:visibleBounds[0],y:visibleBounds[1],width:visibleBounds[2],height:visibleBounds[3]) : .null }
    var rect: CGRect { bounds.count == 4 ? CGRect(x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3]) : .zero }
}
private struct PromptInfo: Decodable {
    struct Action: Decodable { let id: String; let label: String; let destructive: Bool; let cancel: Bool }
    let id: String; let sheet: Bool; let title: String; let message: String; let actions: [Action]
}
private struct Backdrop: Decodable { let color: [CGFloat]; let radius: CGFloat }
/// `target` is the control the action presses; none opens the node's menu.
private struct NodeAction: Decodable { let label: String; let target: String? }
fileprivate struct NavButton: Decodable, Equatable { let id: String; let label: String }
fileprivate struct TabItemInfo: Decodable, Equatable {
    let id: String; let label: String; let symbol: String?; let icon: String; let badge: String?
}
/// The outermost tab view Rust describes.
fileprivate struct TabsInfo: Decodable, Equatable { let id: String; let selected: Int; let items: [TabItemInfo] }
/// The outermost navigation bar Rust describes.
fileprivate struct NavInfo: Decodable, Equatable {
    let id: String; let title: String; let largeTitle: Bool; let depth: Int
    let back: String?; let buttons: [NavButton]
}
private struct ScrollInfo: Decodable {
    let id: String; let frame: [CGFloat]; let content: [CGFloat]; let offset: [CGFloat]; let axes: [Bool]
    /// The top and bottom insets Rust draws with.
    let insets: [CGFloat]?
    /// Pull to refresh: the container has `on_refresh`, and its work is under way.
    let refresh: Bool?; let refreshing: Bool?
    /// Behind an open modal: it takes no touches.
    let inert: Bool?
    /// Inside something that drags, such as a sheet.
    let inDraggable: Bool?
}
private struct Snapshot: Decodable {
    let nodes: [Node]; let dark: Bool; let frames: UInt64; let backend: String; let animating: Bool?
    let scrollViews: [ScrollInfo]?; let navigation: NavInfo?; let tabs: TabsInfo?
    /// A dialog or sheet is open: it covers the bars.
    let modal: Bool?
    /// Long presses that opened a menu, counted.
    let holds: UInt64?
    /// The field Rust focused for typing, such as a search field that opens
    /// with the keyboard up.
    let editing: String?
    /// An alert or action sheet to show with UIKit's own.
    let prompt: PromptInfo?
    /// Areas a finger drags sideways (rows with swipe actions), in points.
    let sideways: [[CGFloat]]?
    let background: [CGFloat]; let fieldBackground: [CGFloat]; let foreground: [CGFloat]; let accent: [CGFloat]
}
/// `top` laid over `bottom`, fully opaque.
private func opaque(_ top: UIColor, over bottom: UIColor) -> UIColor {
    var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
    var (r2, g2, b2, a2): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
    top.getRed(&r, green: &g, blue: &b, alpha: &a)
    bottom.getRed(&r2, green: &g2, blue: &b2, alpha: &a2)
    return UIColor(red: r * a + r2 * (1 - a), green: g * a + g2 * (1 - a), blue: b * a + b2 * (1 - a), alpha: 1)
}
private func color(_ rgba:[CGFloat])->UIColor {
    guard rgba.count == 4 else {return .systemBackground}
    return UIColor(red:rgba[0],green:rgba[1],blue:rgba[2],alpha:rgba[3])
}
private final class Proxy: UIAccessibilityElement {
    var activate: (() -> Bool)?
    var scroll: ((UIAccessibilityScrollDirection) -> Bool)?
    override func accessibilityActivate() -> Bool { activate?() ?? false }
    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool { scroll?(direction) ?? false }
}
/// An empty UIKit scroll view over one Volt scroll view. Its pan gesture is
/// attached to the canvas, so iOS supplies momentum, bounce, indicators and
/// tap-to-top while Rust draws the content at the reported position.
private final class NativeScroll: UIScrollView {
    var nodeId = ""
    var axes = [false, true]
    /// Rust's offset, which includes the insets.
    var rustOffset = CGPoint.zero
    /// Rust got this scroll view's first insets from the safe area, before
    /// UIKit computed its own; UIKit's reports are the truth afterwards.
    var predicted = false
    weak var canvas: VoltCanvas?
    /// Inside a sheet: a downward drag from its top moves the sheet.
    var inDraggable = false
    override func gestureRecognizerShouldBegin(_ g: UIGestureRecognizer) -> Bool {
        guard super.gestureRecognizerShouldBegin(g) else { return false }
        guard g === panGestureRecognizer, let canvas else { return true }
        // Where the finger went down: by the time a pan is recognised it has
        // moved, and a drag that began on a sheet's header must stay the
        // sheet's even once it crosses into the sheet's list.
        let now = g.location(in: canvas), moved = panGestureRecognizer.translation(in: canvas)
        let start = CGPoint(x: now.x - moved.x, y: now.y - moved.y)
        let velocity = panGestureRecognizer.velocity(in: canvas)
        if inDraggable, velocity.y > abs(velocity.x) {
            // As in iOS sheets: at the top of the list (or with nothing to
            // scroll), pulling down moves the sheet, not the list.
            let top = -adjustedContentInset.top
            let scrolls = contentSize.height > bounds.height - adjustedContentInset.top - adjustedContentInset.bottom + 0.5
            if !scrolls || contentOffset.y <= top + 0.5 { return false }
        }
        return canvas.shouldScroll(self, at: start, velocity: velocity)
    }
}
/// Holds the canvas weakly so the display link never keeps it alive.
/// Canvases by Rust view ID, for wakes that arrive after a view is gone.
@MainActor private var wakeTargets: [UInt64: WeakCanvas] = [:]
private struct WeakCanvas { weak var canvas: VoltCanvas? }
/// Rust calls this on the main queue when an Updater sent state changes.
private let wakeCanvas: volt_wake_fn = { handle in
    MainActor.assumeIsolated { wakeTargets[handle]?.canvas?.applyUpdates() }
}
private final class FrameTarget: NSObject {
    weak var canvas: VoltCanvas?
    init(_ canvas: VoltCanvas) { self.canvas = canvas; super.init() }
    @objc func frame() { canvas?.frame() }
}
/// sRGB components of a UIKit colour for the given traits, clamped to 0...1.
private func rgba(_ color: UIColor, _ traits: UITraitCollection) -> [CGFloat] {
    var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
    color.resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
    return [r, g, b, a].map { min(max($0, 0), 1) }
}
private final class InsetField: UITextField {
    override func textRect(forBounds bounds: CGRect) -> CGRect { bounds.insetBy(dx: 12, dy: 0) }
    override func editingRect(forBounds bounds: CGRect) -> CGRect { textRect(forBounds: bounds) }
}
final class VoltCanvas: UIView, UITextFieldDelegate, UITextViewDelegate, UIScrollViewDelegate {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    private var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    private var handle: UInt64 = 0
    private var nodes: [Node] = []
    private var editingId: String?
    private let field = InsetField()
    /// The editor for text areas: several lines, Return adds a line.
    private let area = UITextView()
    private var editingMultiline = false
    /// The native editor in use.
    private var editor: UIView { editingMultiline ? area : field }
    /// The Done bars above the keyboard, which a chat's message box leaves out.
    private let fieldBar = UIToolbar()
    private let areaBar = UIToolbar()
    /// The page colour, for laying a translucent field colour over.
    private var pageBackground: UIColor = .systemBackground
    /// `done()` is running: resigning the editor mustn't call it again.
    private var ending = false
    /// The field being typed in has no Done bar: dragging a list puts the
    /// keyboard away instead.
    private var dragDismisses = false
    /// How far the keyboard covers the canvas, in points.
    private var keyboardOverlap: CGFloat = 0
    /// A sheet or dialog Volt draws is open.
    private var modalOpen = false
    private var sideways: [CGRect] = []
    private var lastEditing: String?
    private var retryCount = 0
    private var suspended = false
    private var failed = false
    private var activeTouch: UITouch?
    private var touchStart = CGPoint.zero
    private var dragged = false
    private var touchFieldId: String?
    /// Fires when the finger rests: a long press opens a menu in Rust.
    private var holdWork: DispatchWorkItem?
    /// Rust's count of long presses that opened a menu; each one buzzes.
    private var holds: UInt64 = 0
    private var touchButtonId: String?
    private var lastSize = CGSize.zero
    private var lastScale: CGFloat = 0
    private let diagnostic = UILabel()
    private var proof: Proxy?
    private var displayLink: CADisplayLink?
    /// The navigation controller that shows this canvas's pages.
    weak var navigation: VoltNavigationController?
    /// The root, for the tab bar.
    weak var root: VoltRootController?
    private var scrollers: [String: NativeScroll] = [:]
    /// Scroll views of pages left behind, kept (with their offsets) so a page
    /// coming back gets the same one and UIKit's large title stays right.
    private var parkedScrollers: [String: NativeScroll] = [:]
    private var parkedOrder: [String] = []
    /// UIKit switches and sliders over Volt's laid-out boxes, each in a
    /// holder clipped to what is visible of its box.
    private var controls: [String: (holder: UIView, control: UIControl)] = [:]
    /// Scroll view IDs from outermost to innermost (layout order).
    private var scrollOrder: [String] = []
    private var syncingScroll = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        isMultipleTouchEnabled = false
        clipsToBounds = true
        isAccessibilityElement = false
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitAccessibilityContrast.self]) { (self: VoltCanvas, _: UITraitCollection) in
            self.sendAppearance()
        }
        NotificationCenter.default.addObserver(self, selector: #selector(sendAppearance), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
        let link = CADisplayLink(target: FrameTarget(self), selector: #selector(FrameTarget.frame))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        link.isPaused = true
        link.add(to: .main, forMode: .common)
        displayLink = link
        metalLayer.device = MTLCreateSystemDefaultDevice()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = false
        metalLayer.isOpaque = true
        // Present with Core Animation so UIKit controls on top move in step,
        // and never stretch an old frame while the view resizes.
        metalLayer.presentsWithTransaction = true
        metalLayer.contentsGravity = .topLeft
        field.delegate = self
        field.font = .systemFont(ofSize: 17)
        field.borderStyle = .none
        field.autocorrectionType = .no
        field.returnKeyType = .done
        field.addTarget(self, action: #selector(edited), for: .editingChanged)
        fieldBar.sizeToFit()
        fieldBar.items = [UIBarButtonItem(systemItem: .flexibleSpace), UIBarButtonItem(title: "Done", style: .done, target: self, action: #selector(done))]
        field.inputAccessoryView = fieldBar
        area.delegate = self
        area.font = .systemFont(ofSize: 17)
        area.textContainerInset = UIEdgeInsets(top: 10, left: 7, bottom: 10, right: 7)
        area.autocorrectionType = .no
        areaBar.sizeToFit()
        areaBar.items = [UIBarButtonItem(systemItem: .flexibleSpace), UIBarButtonItem(title: "Done", style: .done, target: self, action: #selector(done))]
        area.inputAccessoryView = areaBar
        for name in [UIResponder.keyboardWillChangeFrameNotification, UIResponder.keyboardWillHideNotification] {
            NotificationCenter.default.addObserver(self, selector: #selector(keyboardChanged(_:)), name: name, object: nil)
        }
    }
    /// The keyboard covers the bottom of the page: the page's scroll view
    /// gains that much room, as UIKit lists do, and the field scrolls into view.
    /// Nothing is resized, so Volt's content and the field move together.
    @objc private func keyboardChanged(_ note: Notification) {
        guard let end = (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,
              let screen = window?.screen else { return }
        let frame = convert(end, from: screen.coordinateSpace)
        keyboardOverlap = note.name == UIResponder.keyboardWillHideNotification ? 0 : max(0, bounds.maxY - frame.minY)
        // Rust first: a field that rides on the keyboard (a dock's search, a
        // sheet) moves above it, and then needs no scrolling into view.
        sendBars()
        applyKeyboardInset()
    }
    private func applyKeyboardInset() {
        guard let s = contentScroll else { return }
        // Only what the keyboard covers: a transcript above a message box
        // that rides the keyboard (Rust already moved it) needs nothing.
        let covered = keyboardOverlap > 0 ? max(0, s.frame.maxY - (bounds.maxY - keyboardOverlap)) : 0
        let extra = max(0, covered - s.safeAreaInsets.bottom)
        if s.contentInset.bottom != extra { s.contentInset.bottom = extra }
        revealEditor()
    }
    /// Scroll the page so the field being edited sits above the keyboard.
    private func revealEditor() {
        // A field in a sheet: the sheet rises above the keyboard itself, and
        // the page behind it stays where it is.
        guard !modalOpen, let id = editingId, keyboardOverlap > 0, let s = contentScroll,
              let node = nodes.first(where: { $0.id == id }) else { return }
        let top = s.frame.minY + s.adjustedContentInset.top + 8
        let bottom = bounds.height - keyboardOverlap - 16
        let rect = node.rect
        let dy = rect.maxY > bottom ? min(rect.maxY - bottom, rect.minY - top) : (rect.minY < top ? rect.minY - top : 0)
        guard abs(dy) > 0.5 else { return }
        let lowest = -s.adjustedContentInset.top
        let highest = max(lowest, s.contentSize.height - s.bounds.height + s.adjustedContentInset.bottom)
        let y = min(max(s.contentOffset.y + dy, lowest), highest)
        s.setContentOffset(CGPoint(x: s.contentOffset.x, y: y), animated: true)
    }
    required init?(coder: NSCoder) { fatalError("Use programmatic host") }
    deinit {
        displayLink?.invalidate()
        if handle != 0 {
            let id = handle
            MainActor.assumeIsolated { wakeTargets[id] = nil }
            _ = volt_set_wake(id, nil)
            volt_destroy(id)
        }
    }
    /// State changes from other threads are waiting: apply them and draw.
    fileprivate func applyUpdates() { send(["op": "updates"]) }
    /// The system appearance, its colours and the Reduce Motion setting.
    @objc private func sendAppearance() {
        let t = traitCollection
        let colors: [String: [CGFloat]] = [
            "background": rgba(.systemGroupedBackground, t),
            "surface": rgba(.secondarySystemGroupedBackground, t),
            "raised": rgba(.systemGray5, t),
            "label": rgba(.label, t),
            "secondary_label": rgba(.secondaryLabel, t),
            "separator": rgba(.separator, t),
            "tint": rgba(tintColor, t),
            "success": rgba(.systemGreen, t),
            "warning": rgba(.systemOrange, t),
            "error": rgba(.systemRed, t),
        ]
        send(["op": "appearance", "dark": t.userInterfaceStyle == .dark, "colors": colors,
              "reduce_motion": UIAccessibility.isReduceMotionEnabled])
    }
    /// One display-link frame; the link runs only while Rust reports motion.
    fileprivate func frame() {
        guard handle != 0, !failed, !suspended else { displayLink?.isPaused = true; return }
        send(["op": "draw"])
    }
    private func initialize() {
        guard handle == 0, !failed, metalLayer.device != nil else { return }
        guard volt_host_abi_version() == UInt32(VOLT_HOST_ABI_VERSION) else {
            showError("Rust/Apple host ABI mismatch. Rebuild against matching Volt host sources."); return
        }
        // Read the device's system font at runtime. It is never bundled with the app.
        let font = CTFontCreateUIFontForLanguage(.system, 17, nil)!
        guard let url = CTFontCopyAttribute(font, kCTFontURLAttribute) as? URL,
              let data = try? Data(contentsOf: url), !data.isEmpty else {
            showError("The system font could not be loaded.")
            return
        }
        handle = data.withUnsafeBytes { bytes in
            volt_create(Unmanaged.passUnretained(metalLayer).toOpaque(), bytes.bindMemory(to: UInt8.self).baseAddress!, data.count)
        }
        if handle == 0 { showError(String(cString: volt_error())); return }
        wakeTargets[handle] = WeakCanvas(canvas: self)
        _ = volt_set_wake(handle, wakeCanvas)
        send(["op": "host", "native_scrolling": true, "native_navigation": navigation != nil,
              "native_controls": true])
        sendAppearance()
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.width > 0, bounds.height > 0, window != nil, !failed else { return }
        let scale = window?.screen.scale ?? 1
        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        updateTopFade()
        initialize()
        if lastSize != bounds.size || lastScale != scale {
            lastSize = bounds.size; lastScale = scale
            send(["op": "resize", "width": bounds.width, "height": bounds.height, "scale": scale])
        }
        positionEditor()
        sendSafeArea()
        sendBars()
    }
    private var lastSafeArea: UIEdgeInsets?
    /// The window's safe area: the canvas runs under the bars, but sheets and
    /// dialogs Volt draws stay clear of the status bar and home indicator.
    private func sendSafeArea() {
        guard handle != 0, let insets = window?.safeAreaInsets, insets != lastSafeArea else { return }
        lastSafeArea = insets
        send(["op": "safe_area", "top": insets.top, "right": insets.right, "bottom": insets.bottom, "left": insets.left])
    }
    private var lastBars: [CGFloat]?
    /// What covers the canvas from below: the tab bar and home indicator
    /// (the canvas's own safe area), or the keyboard when that is taller.
    /// Volt's action dock sits just above it.
    private func sendBars() {
        let bottom = max(safeAreaInsets.bottom, keyboardOverlap)
        guard handle != 0, [bottom, keyboardOverlap] != lastBars else { return }
        lastBars = [bottom, keyboardOverlap]
        send(["op": "bars", "bottom": bottom, "keyboard": keyboardOverlap])
    }
    override func safeAreaInsetsDidChange() { super.safeAreaInsetsDidChange(); setNeedsLayout() }
    override func didMoveToWindow() { super.didMoveToWindow(); setNeedsLayout() }
    override func tintColorDidChange() { super.tintColorDidChange(); if handle != 0 { sendAppearance() } }
    func suspend(_ value: Bool) {
        suspended = value
        activeTouch = nil
        if value { done() }
        send(["op": "suspend", "value": value])
    }
    /// Run a navigation bar control UIKit drew: Back or a bar button.
    fileprivate func navigate(_ id: String) { send(["op": "navigation", "id": id]) }
    @discardableResult private func send(_ event: [String: Any]) -> Bool {
        precondition(Thread.isMainThread)
        guard handle != 0, !failed, let data = try? JSONSerialization.data(withJSONObject: event) else { return false }
        // A tap may change pages: keep the old look for the page that leaves.
        let op = event["op"] as? String ?? ""
        let before = navigation != nil && ["end", "activate", "navigation"].contains(op)
            ? snapshotView(afterScreenUpdates: false) : nil
        let status = data.withUnsafeBytes { volt_event(handle, $0.bindMemory(to: UInt8.self).baseAddress!, data.count) }
        if status == 0 { showError(String(cString: volt_error())); return false }
        refresh(before: before)
        if status == 2, !suspended, retryCount < 3 {
            retryCount += 1
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.send(["op": "draw"]) }
        } else if status == 2, !suspended { showError("The Metal surface did not recover after three retries.") }
        else if status == 1 { retryCount = 0 }
        return true
    }
    private func refresh(before: UIView? = nil) {
        guard let ptr = volt_snapshot(handle), let data = String(cString: ptr).data(using: .utf8),
              let snapshot = try? JSONDecoder().decode(Snapshot.self, from: data) else { showError("Invalid Rust UI snapshot"); return }
        nodes = snapshot.nodes
        if let count = snapshot.holds, count != holds {
            if count > holds { UIImpactFeedbackGenerator(style: .medium).impactOccurred() }
            holds = count
        }
        root?.sync(snapshot.tabs, before: before, accent: color(snapshot.accent), modal: snapshot.modal ?? false)
        modalOpen = snapshot.modal ?? false
        sideways = (snapshot.sideways ?? []).compactMap { r in
            r.count == 4 ? CGRect(x: r[0], y: r[1], width: r[2], height: r[3]) : nil
        }
        // The field being typed in is gone (a dock's search closed): put the
        // keyboard away rather than type into nothing.
        if let id = editingId, !nodes.contains(where: { $0.id == id }) {
            editingId = nil
            editor.resignFirstResponder()
            editor.removeFromSuperview()
        }
        // Rust asked for a field to be typed in: open the keyboard once, when
        // the request changes, so dismissing the keyboard sticks.
        if snapshot.editing != lastEditing {
            lastEditing = snapshot.editing
            if let want = snapshot.editing, want != editingId {
                // Next turn, if Rust still wants it and the field is still
                // there: a tap that closes search also commits its text.
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.lastEditing == want, self.editingId != want else { return }
                    // UIKit's own search field takes the keyboard itself.
                    if let search = self.controls[want]?.control as? UISearchTextField {
                        if !search.isFirstResponder { search.becomeFirstResponder() }
                        return
                    }
                    guard let node = self.nodes.first(where: { $0.id == want && $0.kind == "field" && !$0.readOnly })
                    else { return }
                    self.beginEditing(node)
                }
            }
        }
        syncPrompt(snapshot.prompt)
        navigation?.cover(modalOpen)
        pageBackground = color(snapshot.background)
        navigation?.sync(snapshot.navigation, before: before, background: color(snapshot.background),
                         accent: color(snapshot.accent))
        displayLink?.isPaused = !(snapshot.animating ?? false) || suspended
        syncScrollViews(snapshot.scrollViews ?? [])
        updateTopFade()
        var elements: [Any] = []
        // Scroll is handled by the container's accessibilityScroll override.
        // A full-viewport leaf proxy here would occlude its real editable child
        // in accessibility hit-testing even though that child is visible.
        syncControls()
        for node in nodes where node.kind == "switch" || node.kind == "slider" || node.kind == "search" {
            if let control = controls[node.id]?.control, !(controls[node.id]?.holder.isHidden ?? true) {
                elements.append(control)
            }
        }
        for node in nodes where !node.label.isEmpty && node.kind != "scroll" && node.kind != "switch" && node.kind != "slider" && node.kind != "search" {
            let rect = node.visibleRect.intersection(bounds)
            guard !rect.isNull, rect.width > 1, rect.height > 1 else { continue }
            if node.id == editingId { elements.append(editor); continue }
            let proxy = Proxy(accessibilityContainer: self)
            proxy.accessibilityIdentifier = node.id
            proxy.accessibilityLabel = node.label.isEmpty ? "Scrollable content" : node.label
            proxy.accessibilityValue = node.value
            proxy.accessibilityFrameInContainerSpace = rect
            proxy.accessibilityTraits = node.kind == "button" || node.kind == "field" ? .button : .staticText
            if !node.enabled { proxy.accessibilityTraits.insert(.notEnabled) }
            if node.kind == "button" || node.kind == "field" {
                proxy.activate = { [weak self] in self?.activate(node) ?? false }
            }
            proxy.accessibilityCustomActions = (node.actions ?? []).map { action in
                UIAccessibilityCustomAction(name: action.label) { [weak self] _ in
                    guard let self else { return false }
                    if let target = action.target { return self.send(["op": "activate", "id": target]) }
                    return self.send(["op": "menu", "id": node.id])
                }
            }
            elements.append(proxy)
        }
        // An invisible, read-only diagnostic for automated acceptance; no timer.
        let p = proof ?? Proxy(accessibilityContainer: self)
        p.accessibilityIdentifier = "render-proof"
        p.accessibilityLabel = "Renderer"
        p.accessibilityValue = "\(snapshot.backend); frames=\(snapshot.frames)"
        p.accessibilityFrameInContainerSpace = CGRect(x:0,y:0,width:1,height:1)
        proof = p; elements.append(p)
        accessibilityElements = elements
        field.textColor = color(snapshot.foreground)
        field.backgroundColor = color(snapshot.fieldBackground)
        field.tintColor = color(snapshot.accent)
        area.textColor = field.textColor
        area.backgroundColor = field.backgroundColor
        area.tintColor = field.tintColor
        positionEditor()
        // A new page learns its bars' insets before anyone sees a frame.
        let insets = pendingInsets
        pendingInsets.removeAll()
        guard refreshDepth < 4 else { return }
        refreshDepth += 1
        defer { refreshDepth -= 1 }
        for (id, top, bottom) in insets {
            send(["op": "scroll_insets", "id": id, "top": top, "bottom": bottom])
        }
    }
    /// Match one UIKit scroll view to each Volt scroll view. Rust's position wins
    /// only while the person isn't scrolling (keyboard reveal, restored state).
    private func syncScrollViews(_ infos: [ScrollInfo]) {
        syncingScroll = true
        defer { syncingScroll = false }
        var seen = Set<String>()
        var topSet = false
        let valid = infos.filter { $0.frame.count == 4 && $0.content.count == 2 && $0.offset.count == 2 && $0.axes.count == 2 }
        // The page's main scroll view starts at the top of the canvas and
        // spans it: it runs under the bars, which UIKit insets. Decide before
        // positioning, so its first offset already accounts for the bar.
        let mainId = valid.first { $0.axes[1] && $0.frame[1] < 0.5 && $0.frame[2] >= bounds.width - 1 }?.id
        for info in valid {
            seen.insert(info.id)
            let s = scrollers[info.id] ?? unpark(info.id) ?? makeScroller(info.id)
            s.axes = info.axes
            // Behind a modal its pan (which the canvas carries) stays off.
            let live = !(info.inert ?? false)
            s.inDraggable = info.inDraggable ?? false
            if s.panGestureRecognizer.isEnabled != live { s.panGestureRecognizer.isEnabled = live }
            let behavior: UIScrollView.ContentInsetAdjustmentBehavior = info.id == mainId ? .always : .never
            if s.contentInsetAdjustmentBehavior != behavior { s.contentInsetAdjustmentBehavior = behavior }
            let frame = CGRect(x: info.frame[0], y: info.frame[1], width: info.frame[2], height: info.frame[3])
            if s.frame != frame { s.frame = frame }
            let size = CGSize(width: info.axes[0] ? info.content[0] : frame.width,
                              height: info.axes[1] ? info.content[1] : frame.height)
            if s.contentSize != size { s.contentSize = size }
            s.alwaysBounceVertical = info.axes[1]
            s.alwaysBounceHorizontal = info.axes[0]
            // The status-bar tap scrolls the outermost vertical scroll view.
            s.scrollsToTop = info.axes[1] && !topSet
            topSet = topSet || info.axes[1]
            s.rustOffset = CGPoint(x: info.offset[0], y: info.offset[1])
            let drawnTop = info.insets?.first ?? 0
            if info.id == mainId, !s.predicted {
                // The bars a new page runs under, before its first frame:
                // the safe area, plus any keyboard room. Once only.
                s.predicted = true
                let top = safeAreaInsets.top, bottom = safeAreaInsets.bottom + s.contentInset.bottom
                let have = info.insets ?? [0, 0]
                if have.count == 2, abs(have[0] - top) > 0.5 || abs(have[1] - bottom) > 0.5 {
                    pendingInsets.append((info.id, top, bottom))
                }
            }
            syncRefresh(s, info)
            // A refresh control holds the content down while it spins; leave it.
            let spinning = s.refreshControl?.isRefreshing ?? false
            if !s.isTracking && !s.isDragging && !s.isDecelerating && !spinning {
                // Where Rust draws the content: at drawnTop - offset.
                let target = CGPoint(x: info.offset[0], y: info.offset[1] - drawnTop)
                if abs(s.contentOffset.x - target.x) > 0.5 || abs(s.contentOffset.y - target.y) > 0.5 {
                    s.contentOffset = target
                }
            }
        }
        for (id, s) in scrollers where !seen.contains(id) {
            removeGestureRecognizer(s.panGestureRecognizer)
            s.removeFromSuperview()
            scrollers[id] = nil
            parkedScrollers[id] = s
            parkedOrder.removeAll { $0 == id }
            parkedOrder.append(id)
            if parkedOrder.count > 32 { parkedScrollers[parkedOrder.removeFirst()] = nil }
        }
        scrollOrder = infos.map(\.id).filter { seen.contains($0) }
        let main = mainId.flatMap { scrollers[$0] }
        if main !== contentScroll {
            let old = contentScroll
            contentScroll = main
            DispatchQueue.main.async { [weak self] in
                old?.contentInset.bottom = 0
                self?.navigation?.setContent(self?.contentScroll)
                self?.applyKeyboardInset()
            }
        }
    }
    private weak var contentScroll: NativeScroll?
    /// The alert or action sheet UIKit is showing for Rust, by its ID.
    private var shownPrompt: (id: String, controller: UIAlertController)?
    /// Show Rust's alert or action sheet with UIKit's own, and put it away
    /// when Rust closes it. Every button runs Rust's, which closes it there.
    private func syncPrompt(_ prompt: PromptInfo?) {
        if let shown = shownPrompt, shown.id != prompt?.id {
            if shown.controller.presentingViewController != nil { shown.controller.dismiss(animated: true) }
            shownPrompt = nil
        }
        guard let prompt, shownPrompt == nil, !suspended else { return }
        var top = window?.rootViewController
        while let next = top?.presentedViewController { top = next }
        guard let host = top else { return }
        let alert = UIAlertController(title: prompt.title.isEmpty ? nil : prompt.title,
                                      message: prompt.message.isEmpty ? nil : prompt.message,
                                      preferredStyle: prompt.sheet ? .actionSheet : .alert)
        for action in prompt.actions {
            let style: UIAlertAction.Style = action.cancel ? .cancel : action.destructive ? .destructive : .default
            alert.addAction(UIAlertAction(title: action.label, style: style) { [weak self] _ in
                guard let self else { return }
                self.shownPrompt = nil
                self.send(["op": "navigation", "id": action.id])
            })
        }
        // An action sheet grows from what opened it: where the finger was.
        if let popover = alert.popoverPresentationController {
            popover.sourceView = self
            popover.sourceRect = CGRect(x: touchStart.x, y: touchStart.y, width: 1, height: 1)
        }
        shownPrompt = (prompt.id, alert)
        host.present(alert, animated: true)
    }
    /// Content scrolled under the status and navigation bars fades into the
    /// page, as it does in UIKit lists. UIKit's own edge effect can't fade
    /// Volt's drawing (the scroll views it watches are empty stand-ins), so
    /// the host lays this gradient over the canvas.
    private let topFade: UIView = {
        let v = UIView()
        v.isUserInteractionEnabled = false
        v.layer.addSublayer(CAGradientLayer())
        return v
    }()
    private func updateTopFade() {
        // Only where a main scroll view runs under the bars, and not while a
        // sheet or alert covers the page.
        guard let s = contentScroll, !modalOpen, s.adjustedContentInset.top > 1 else {
            topFade.isHidden = true
            return
        }
        // Only the status bar and the compact navigation bar: a large title
        // stays clear. It shows as content scrolls under, not at rest.
        let bars = (window?.safeAreaInsets.top ?? safeAreaInsets.top) + 54
        let height = bars + 16
        let scrolled = s.contentOffset.y + s.adjustedContentInset.top
        if topFade.superview !== self { addSubview(topFade) }
        // Under the native text editor, above the drawing.
        if editingId != nil { insertSubview(topFade, belowSubview: editor) } else { bringSubviewToFront(topFade) }
        topFade.isHidden = scrolled <= 0.5
        topFade.alpha = min(1, max(0, scrolled / 24))
        topFade.frame = CGRect(x: 0, y: 0, width: bounds.width, height: height)
        guard let g = topFade.layer.sublayers?.first as? CAGradientLayer else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        g.frame = topFade.bounds
        g.colors = [pageBackground.withAlphaComponent(0.9), pageBackground.withAlphaComponent(0.6), pageBackground.withAlphaComponent(0)].map(\.cgColor)
        g.locations = [0, NSNumber(value: Double((bars - 20) / height)), 1]
        CATransaction.commit()
    }
    /// Insets to give Rust once the current update finishes.
    private var pendingInsets: [(String, CGFloat, CGFloat)] = []
    /// Nested updates in progress; bounded so no feedback can recurse.
    private var refreshDepth = 0
    /// Rust offsets include the insets: 0 is the top of the inset area.
    private func rustOffset(_ s: UIScrollView) -> CGPoint {
        CGPoint(x: s.contentOffset.x + s.adjustedContentInset.left, y: s.contentOffset.y + s.adjustedContentInset.top)
    }
    func scrollViewDidChangeAdjustedContentInset(_ scrollView: UIScrollView) {
        // Off screen (parked, or the canvas moving between pages) the safe
        // area reads zero; that is not a change in the bars.
        guard let s = scrollView as? NativeScroll, s.window != nil, scrollers[s.nodeId] === s else { return }
        // A page at the top stays at the top as its bar changes (the large
        // title shows). Anywhere else UIKit has already kept the content in
        // place; moving it again would make it creep.
        if !s.isTracking, !s.isDragging, !s.isDecelerating, s.rustOffset.y <= 0.5 {
            let was = syncingScroll
            syncingScroll = true
            s.contentOffset.y = -s.adjustedContentInset.top
            syncingScroll = was
        }
        let report = { [weak self, weak s] in
            guard let self, let s, s.window != nil, self.scrollers[s.nodeId] === s, self.refreshDepth < 4 else { return }
            self.refreshDepth += 1
            defer { self.refreshDepth -= 1 }
            let inset = s.adjustedContentInset
            self.send(["op": "scroll_insets", "id": s.nodeId, "top": inset.top, "bottom": inset.bottom])
            let o = self.rustOffset(s)
            self.send(["op": "scroll_to", "id": s.nodeId, "x": o.x, "y": o.y])
        }
        // Mid-update (views being matched to a snapshot), report afterwards.
        if syncingScroll { DispatchQueue.main.async(execute: report) } else { report() }
    }
    private func syncControls() {
        var seen = Set<String>()
        for node in nodes where node.kind == "switch" || node.kind == "slider" || node.kind == "search" {
            seen.insert(node.id)
            let entry = controls[node.id] ?? makeControl(node)
            let visible = node.visibleRect
            entry.holder.isHidden = visible.isNull
            guard !visible.isNull else { continue }
            entry.control.isEnabled = node.enabled
            entry.control.accessibilityIdentifier = node.id
            entry.control.accessibilityLabel = node.label
            if let toggle = entry.control as? UISwitch {
                // The size UIKit draws it at: iOS 26's switch is wider than
                // older ones and than its intrinsic size says.
                toggle.sizeToFit()
                let size = toggle.bounds.size
                // Without a visible label the switch sits at its box's
                // trailing edge, as in an iOS row.
                let box = node.rect
                let x = box.width < size.width + 8 ? box.maxX - size.width : box.minX + 2
                let frame = CGRect(x: x, y: box.midY - size.height / 2, width: size.width, height: size.height)
                // A switch a little wider than its box still draws whole,
                // rounded ends and all; only a scroll view's edge cuts it.
                let holder = visible == box ? box.union(frame) : visible
                entry.holder.frame = holder
                toggle.frame = frame.offsetBy(dx: -holder.minX, dy: -holder.minY)
                let on = node.checked ?? false
                if toggle.isOn != on { toggle.setOn(on, animated: true) }
            } else if let search = entry.control as? UISearchTextField {
                // UIKit's own search field, centred in the space Volt gave it.
                entry.holder.frame = visible
                let rect = node.rect.offsetBy(dx: -visible.minX, dy: -visible.minY)
                let height = min(rect.height, 44)
                search.frame = CGRect(x: rect.minX, y: rect.midY - height / 2, width: rect.width, height: height)
                search.placeholder = node.placeholder ?? node.label
                // Rust's query, unless someone is typing in it now.
                if !search.isFirstResponder, search.text != (node.value ?? "") { search.text = node.value ?? "" }
            } else if let slider = entry.control as? UISlider, let n = node.numeric, n.count == 4 {
                entry.holder.frame = visible
                let rect = node.rect.offsetBy(dx: -visible.minX, dy: -visible.minY)
                let height = max(slider.intrinsicContentSize.height, 34)
                slider.frame = CGRect(x: rect.minX, y: rect.midY - height / 2, width: rect.width, height: height)
                slider.minimumValue = Float(n[1])
                slider.maximumValue = Float(n[2])
                if !slider.isTracking, slider.value != Float(n[0]) { slider.value = Float(n[0]) }
            }
        }
        for (id, entry) in controls where !seen.contains(id) {
            entry.holder.removeFromSuperview()
            controls[id] = nil
        }
    }
    private func makeControl(_ node: Node) -> (holder: UIView, control: UIControl) {
        let holder = UIView()
        holder.clipsToBounds = true
        let control: UIControl
        if node.kind == "search" {
            let search = UISearchTextField()
            search.returnKeyType = .search
            search.autocorrectionType = .no
            search.text = node.value ?? ""
            search.addAction(UIAction { [weak self, weak search] _ in
                guard let self, let search, search.markedTextRange == nil else { return }
                self.send(["op": "text", "id": node.id, "text": search.text ?? ""])
            }, for: .editingChanged)
            search.addAction(UIAction { [weak self] _ in
                self?.send(["op": "focus", "id": node.id])
            }, for: .editingDidBegin)
            // The Search key runs the search and puts the keyboard away.
            search.addAction(UIAction { [weak self] _ in
                self?.send(["op": "submit", "id": node.id])
            }, for: .editingDidEndOnExit)
            search.addAction(UIAction { [weak self] _ in
                self?.send(["op": "blur"])
            }, for: .editingDidEnd)
            control = search
        } else if node.kind == "switch" {
            let toggle = UISwitch()
            toggle.addAction(UIAction { [weak self] _ in self?.send(["op": "activate", "id": node.id]) }, for: .valueChanged)
            control = toggle
        } else {
            let slider = UISlider()
            slider.isContinuous = true
            slider.addAction(UIAction { [weak self, weak slider] _ in
                guard let slider else { return }
                self?.send(["op": "value", "id": node.id, "value": slider.value])
            }, for: .valueChanged)
            // A sideways drag on the slider must not start a back swipe.
            slider.addAction(UIAction { [weak self] _ in self?.navigation?.holdBackGestures(true) }, for: .touchDown)
            slider.addAction(UIAction { [weak self] _ in self?.navigation?.holdBackGestures(false) },
                             for: [.touchUpInside, .touchUpOutside, .touchCancel])
            control = slider
        }
        holder.addSubview(control)
        addSubview(holder)
        let entry = (holder: holder, control: control)
        controls[node.id] = entry
        return entry
    }
    private func unpark(_ id: String) -> NativeScroll? {
        guard let s = parkedScrollers.removeValue(forKey: id) else { return nil }
        parkedOrder.removeAll { $0 == id }
        insertSubview(s, at: 0)
        addGestureRecognizer(s.panGestureRecognizer)
        scrollers[id] = s
        return s
    }
    /// Pull to refresh: the system's control on a scroll view whose container
    /// has `on_refresh`. It stays while Rust says the work is under way, and
    /// closes when it isn't (at once, if `on_refresh` finished immediately).
    private func syncRefresh(_ s: NativeScroll, _ info: ScrollInfo) {
        guard info.refresh == true else {
            if s.refreshControl != nil { s.refreshControl = nil }
            return
        }
        let control: UIRefreshControl
        if let existing = s.refreshControl {
            control = existing
        } else {
            control = UIRefreshControl()
            control.addTarget(self, action: #selector(pulledToRefresh(_:)), for: .valueChanged)
            s.refreshControl = control
        }
        if info.refreshing != true, control.isRefreshing { control.endRefreshing() }
    }
    @objc private func pulledToRefresh(_ control: UIRefreshControl) {
        guard let s = control.superview as? NativeScroll else { return }
        send(["op": "refresh", "id": s.nodeId])
    }
    private func makeScroller(_ id: String) -> NativeScroll {
        let s = NativeScroll()
        s.nodeId = id
        s.canvas = self
        s.delegate = self
        s.backgroundColor = .clear
        s.contentInsetAdjustmentBehavior = .never
        // Touches reach the canvas; only the pan gesture moves to it.
        s.isUserInteractionEnabled = false
        insertSubview(s, at: 0)
        addGestureRecognizer(s.panGestureRecognizer)
        scrollers[id] = s
        return s
    }
    /// A scroll view starts a pan under its frame unless an inner one there
    /// scrolls in the pan's direction.
    fileprivate func shouldScroll(_ s: NativeScroll, at p: CGPoint, velocity v: CGPoint) -> Bool {
        guard s.frame.contains(p) else { return false }
        let axis = abs(v.y) >= abs(v.x) ? 1 : 0
        guard s.axes[axis] else { return false }
        guard let i = scrollOrder.firstIndex(of: s.nodeId) else { return true }
        return !scrollOrder[(i + 1)...].contains { id in
            guard let inner = scrollers[id] else { return false }
            return inner.frame.contains(p) && inner.axes[axis]
        }
    }
    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        if dragDismisses, editingId != nil, keyboardOverlap > 0 { done() }
    }
    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        navigation?.holdContentBackGesture(false)
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        guard !syncingScroll, let s = scrollView as? NativeScroll, s.window != nil, scrollers[s.nodeId] === s else { return }
        let o = rustOffset(s)
        send(["op": "scroll_to", "id": s.nodeId, "x": o.x, "y": o.y])
    }
    private func showError(_ message: String) {
        failed = true
        field.resignFirstResponder()
        diagnostic.text = "Volt mobile host\n\n\(message)"
        diagnostic.numberOfLines = 0
        diagnostic.textColor = .systemRed
        diagnostic.backgroundColor = .systemBackground
        diagnostic.frame = bounds.insetBy(dx:20,dy:20)
        diagnostic.accessibilityIdentifier = "host-error"
        addSubview(diagnostic)
        accessibilityElements = [diagnostic]
        NSLog("Volt iOS error: %@",message)
    }
    private func activate(_ node: Node) -> Bool {
        guard node.enabled else { return false }
        if node.kind == "field" { guard !node.readOnly else {return false}; beginEditing(node); return true }
        done()
        let success = send(["op":"activate", "id":node.id])
        UIAccessibility.post(notification: .layoutChanged, argument: nil)
        return success
    }
    private func beginEditing(_ node: Node) {
        if editingId != node.id { done() }
        editingId = node.id
        editingMultiline = node.multiline ?? false
        // A message box has no Done bar: dragging the conversation puts the
        // keyboard away, as in Messages.
        let bar = node.doneBar ?? true
        field.inputAccessoryView = bar ? fieldBar : nil
        area.inputAccessoryView = bar ? areaBar : nil
        dragDismisses = !bar
        send(["op":"focus", "id":node.id])
        if editingMultiline {
            area.text = node.value ?? ""
            area.autocapitalizationType = .sentences
        } else {
            field.text = node.value ?? ""
            field.isSecureTextEntry = node.password
            field.autocapitalizationType = .none
            field.keyboardType = .default
        }
        editor.accessibilityIdentifier = "editor-\(node.id)"
        editor.accessibilityLabel = node.label
        addSubview(editor)
        positionEditor()
        editor.becomeFirstResponder()
        refresh()
        // Already open: the keyboard sends no new frame for the next field.
        revealEditor()
    }
    private func positionEditor() {
        guard let id = editingId else { return }
        guard let node = nodes.first(where: { $0.id == id }) else { editingId = nil; editor.resignFirstResponder(); editor.removeFromSuperview(); return }
        editor.frame = node.rect.insetBy(dx:2,dy:2)
        // Blend in with what's behind the field: a message box's pill, a
        // dock's search capsule.
        if let b = node.backdrop {
            // Opaque, over the page: Volt's own drawing of the text must not
            // show through a glassy pill.
            editor.backgroundColor = opaque(color(b.color), over: pageBackground)
            editor.layer.cornerRadius = min(max(0, b.radius - 2), editor.frame.height / 2)
        } else {
            editor.backgroundColor = editingMultiline ? area.backgroundColor : field.backgroundColor
            editor.layer.cornerRadius = 10
        }
        editor.clipsToBounds = true
    }
    @objc private func edited() {
        guard field.markedTextRange == nil, let id = editingId else { return }
        send(["op":"text", "id":id, "text":field.text ?? ""])
    }
    /// The keyboard went away some other way (dragged down with the page):
    /// commit the text, as Done does.
    func textViewDidEndEditing(_ textView: UITextView) { if editingId != nil { done() } }
    func textFieldDidEndEditing(_ textField: UITextField) { if editingId != nil { done() } }
    func textViewDidChange(_ textView: UITextView) {
        guard textView.markedTextRange == nil, let id = editingId else { return }
        send(["op":"text", "id":id, "text":textView.text ?? ""])
    }
    @objc private func done() {
        guard let id = editingId, !ending else { return }
        ending = true
        defer { ending = false }
        // Resigning commits an active IME composition before notifying Rust.
        editor.resignFirstResponder()
        let committed = editingMultiline ? area.text ?? "" : field.text ?? ""
        let closing = editor
        editingId = nil
        send(["op":"text", "id":id, "text":committed])
        // Rust lets go of the field too, so nothing reopens the keyboard
        // until someone taps a field again.
        send(["op":"blur"])
        closing.removeFromSuperview()
        refreshIfMounted()
    }
    private func refreshIfMounted() { if handle != 0, !failed { refresh() } }
    func textFieldShouldReturn(_ textField: UITextField) -> Bool { done(); return true }
    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
        guard direction == .up || direction == .down,
              let node = nodes.first(where: { $0.kind == "scroll" && $0.rect.intersects(bounds) }) else { return false }
        let changed = send(["op":"scroll", "id":node.id, "dy":(direction == .down ? 1 : -1) * max(80,bounds.height-120)])
        if changed { UIAccessibility.post(notification: .pageScrolled, argument: "Content scrolled") }
        return changed
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard !suspended, activeTouch == nil, let t = touches.first else { return }
        activeTouch = t; touchStart = t.location(in:self); dragged = false
        touchButtonId = nodes.last(where: { $0.kind == "button" && $0.enabled && $0.visibleRect.contains(touchStart) })?.id
        // The field under the finger now: pressing it can open the keyboard
        // and move the field (a message box rides up) before the finger lifts.
        touchFieldId = nodes.last(where: { $0.kind == "field" && $0.enabled && !$0.readOnly && $0.visibleRect.contains(touchStart) })?.id
        // A sideways carousel or a swipeable row under the finger moves
        // instead of going back.
        if scrollers.values.contains(where: { $0.axes[0] && !$0.axes[1] && $0.frame.contains(touchStart) })
            || sideways.contains(where: { $0.contains(touchStart) }) {
            navigation?.holdContentBackGesture(true)
        }
        send(["op":"begin", "x":touchStart.x,"y":touchStart.y])
        holdWork?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.hold(t) }
        holdWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.45, execute: work)
    }
    /// The finger stayed put: ask Rust for the menu under it. If one opens,
    /// this touch is over; lifting the finger presses nothing.
    private func hold(_ t: UITouch) {
        holdWork = nil
        guard activeTouch === t, !dragged, !suspended, editingId == nil else { return }
        let before = holds
        send(["op":"hold", "x":touchStart.x, "y":touchStart.y])
        guard holds != before else { return }
        activeTouch = nil
        navigation?.holdContentBackGesture(false)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = activeTouch, touches.contains(t) else { return }
        let p = t.location(in:self)
        dragged = dragged || hypot(p.x-touchStart.x,p.y-touchStart.y) >= 8
        if dragged { holdWork?.cancel(); holdWork = nil }
        send(["op":"move", "x":p.x,"y":p.y])
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        holdWork?.cancel(); holdWork = nil
        guard let t = activeTouch, touches.contains(t) else { return }
        let p = t.location(in:self)
        dragged = dragged || hypot(p.x-touchStart.x,p.y-touchStart.y) >= 8
        let target = dragged ? nil
            : touchFieldId.flatMap { id in nodes.first(where: { $0.id == id }) }
                ?? nodes.last(where: { $0.kind == "field" && $0.enabled && !$0.readOnly && $0.visibleRect.contains(p) })
        activeTouch = nil
        navigation?.holdContentBackGesture(false)
        if editingId != nil, !dragged,
           let button = nodes.last(where: { $0.kind == "button" && $0.enabled && $0.visibleRect.contains(p) }), button.id == touchButtonId {
            // Committing the native editor recomposes Rust and cancels its old
            // pointer capture. Treat commit + this same-control tap atomically;
            // never require a second tap on Cancel/Save while editing.
            send(["op":"cancel"])
            if button.keepsKeyboard ?? false, let id = editingId {
                // Send in a message box: commit, press, and keep typing in
                // the same field, now holding whatever Rust left there.
                let text = editingMultiline ? area.text ?? "" : field.text ?? ""
                send(["op":"text", "id":id, "text":text])
                send(["op":"activate", "id":button.id])
                guard nodes.contains(where: { $0.id == id }) else { done(); return }
                send(["op":"focus", "id":id])
                let value = nodes.first(where: { $0.id == id })?.value ?? ""
                if editingMultiline { area.text = value } else { field.text = value }
                positionEditor()
                return
            }
            done()
            send(["op":"activate", "id":button.id])
            return
        }
        if target == nil { done() }
        send(["op":"end", "x":p.x,"y":p.y])
        if let target { beginEditing(target) }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        holdWork?.cancel(); holdWork = nil
        activeTouch = nil; send(["op":"cancel"])
        // A carousel that took the touch releases the hold when it stops dragging.
        if !scrollers.values.contains(where: { $0.isTracking }) { navigation?.holdContentBackGesture(false) }
    }
}
