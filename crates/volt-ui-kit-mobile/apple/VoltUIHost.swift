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
        window.rootViewController = VoltHostController()
        window.makeKeyAndVisible()
        self.window = window
    }
    func sceneWillResignActive(_ scene: UIScene) { (window?.rootViewController as? VoltHostController)?.canvas.suspend(true) }
    func sceneDidBecomeActive(_ scene: UIScene) { (window?.rootViewController as? VoltHostController)?.canvas.suspend(false) }
}
final class VoltHostController: UIViewController {
    let canvas = VoltCanvas()
    override func viewDidLoad() {
        super.viewDidLoad()
        view.addSubview(canvas)
        canvas.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            canvas.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            canvas.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            canvas.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor)
        ])
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: VoltHostController, _: UITraitCollection) in self.updateColors() }
        updateColors()
    }
    func updateColors() {
        view.backgroundColor = traitCollection.userInterfaceStyle == .dark ? .black : .white
    }
}
private struct Node: Decodable {
    let id: String
    let label: String
    let value: String?
    let enabled: Bool
    let kind: String
    let password: Bool
    let readOnly: Bool
    let bounds: [CGFloat]
    let visibleBounds: [CGFloat]
    var visibleRect: CGRect { visibleBounds.count == 4 && visibleBounds[2] > 0 && visibleBounds[3] > 0 ? CGRect(x:visibleBounds[0],y:visibleBounds[1],width:visibleBounds[2],height:visibleBounds[3]) : .null }
    var rect: CGRect { bounds.count == 4 ? CGRect(x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3]) : .zero }
}
private struct Snapshot: Decodable {
    let nodes: [Node]; let dark: Bool; let frames: UInt64; let backend: String
    let background: [CGFloat]; let fieldBackground: [CGFloat]; let foreground: [CGFloat]; let accent: [CGFloat]
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
private final class InsetField: UITextField {
    override func textRect(forBounds bounds: CGRect) -> CGRect { bounds.insetBy(dx: 12, dy: 0) }
    override func editingRect(forBounds bounds: CGRect) -> CGRect { textRect(forBounds: bounds) }
}
final class VoltCanvas: UIView, UITextFieldDelegate {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    private var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    private var handle: UInt64 = 0
    private var nodes: [Node] = []
    private var editingId: String?
    private let field = InsetField()
    private var retryCount = 0
    private var suspended = false
    private var failed = false
    private var activeTouch: UITouch?
    private var touchStart = CGPoint.zero
    private var dragged = false
    private var touchButtonId: String?
    private var lastSize = CGSize.zero
    private var lastScale: CGFloat = 0
    private let diagnostic = UILabel()
    private var proof: Proxy?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isMultipleTouchEnabled = false
        clipsToBounds = true
        isAccessibilityElement = false
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: VoltCanvas, _: UITraitCollection) in
            self.send(["op":"appearance", "dark":self.traitCollection.userInterfaceStyle == .dark])
        }
        metalLayer.device = MTLCreateSystemDefaultDevice()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = false
        metalLayer.isOpaque = true
        field.delegate = self
        field.font = .systemFont(ofSize: 17)
        field.borderStyle = .none
        field.autocorrectionType = .no
        field.returnKeyType = .done
        field.addTarget(self, action: #selector(edited), for: .editingChanged)
        let toolbar = UIToolbar()
        toolbar.sizeToFit()
        toolbar.items = [UIBarButtonItem(systemItem: .flexibleSpace), UIBarButtonItem(title: "Done", style: .done, target: self, action: #selector(done))]
        field.inputAccessoryView = toolbar
    }
    required init?(coder: NSCoder) { fatalError("Use programmatic host") }
    deinit { if handle != 0 { volt_destroy(handle) } }
    private func initialize() {
        guard handle == 0, !failed, metalLayer.device != nil else { return }
        guard volt_host_abi_version() == UInt32(VOLT_HOST_ABI_VERSION) else {
            showError("Rust/Apple host ABI mismatch. Rebuild against matching Volt host sources."); return
        }
        // Read Apple's installed font locally. Never bundle/redistribute SF.
        let font = CTFontCreateUIFontForLanguage(.system, 17, nil)!
        guard let url = CTFontCopyAttribute(font, kCTFontURLAttribute) as? URL,
              let data = try? Data(contentsOf: url), !data.isEmpty else {
            showError("The system SF font could not be loaded.")
            return
        }
        handle = data.withUnsafeBytes { bytes in
            volt_create(Unmanaged.passUnretained(metalLayer).toOpaque(), bytes.bindMemory(to: UInt8.self).baseAddress!, data.count)
        }
        if handle == 0 { showError(String(cString: volt_error())); return }
        send(["op": "appearance", "dark": traitCollection.userInterfaceStyle == .dark])
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.width > 0, bounds.height > 0, window != nil, !failed else { return }
        let scale = window?.screen.scale ?? 1
        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        initialize()
        if lastSize != bounds.size || lastScale != scale {
            lastSize = bounds.size; lastScale = scale
            send(["op": "resize", "width": bounds.width, "height": bounds.height, "scale": scale])
        }
        positionEditor()
    }
    override func didMoveToWindow() { super.didMoveToWindow(); setNeedsLayout() }
    func suspend(_ value: Bool) {
        suspended = value
        activeTouch = nil
        if value { done() }
        send(["op": "suspend", "value": value])
    }
    @discardableResult private func send(_ event: [String: Any]) -> Bool {
        precondition(Thread.isMainThread)
        guard handle != 0, !failed, let data = try? JSONSerialization.data(withJSONObject: event) else { return false }
        let status = data.withUnsafeBytes { volt_event(handle, $0.bindMemory(to: UInt8.self).baseAddress!, data.count) }
        if status == 0 { showError(String(cString: volt_error())); return false }
        refresh()
        if status == 2, !suspended, retryCount < 3 {
            retryCount += 1
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.send(["op": "draw"]) }
        } else if status == 2, !suspended { showError("The Metal surface did not recover after three retries.") }
        else if status == 1 { retryCount = 0 }
        return true
    }
    private func refresh() {
        guard let ptr = volt_snapshot(handle), let data = String(cString: ptr).data(using: .utf8),
              let snapshot = try? JSONDecoder().decode(Snapshot.self, from: data) else { showError("Invalid Rust UI snapshot"); return }
        nodes = snapshot.nodes
        var elements: [Any] = []
        // Scroll is handled by the container's accessibilityScroll override.
        // A full-viewport leaf proxy here would occlude its real editable child
        // in accessibility hit-testing even though that child is visible.
        for node in nodes where !node.label.isEmpty && node.kind != "scroll" {
            let rect = node.visibleRect.intersection(bounds)
            guard !rect.isNull, rect.width > 1, rect.height > 1 else { continue }
            if node.id == editingId { elements.append(field); continue }
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
        superview?.backgroundColor = color(snapshot.background)
        field.textColor = color(snapshot.foreground)
        field.backgroundColor = color(snapshot.fieldBackground)
        field.tintColor = color(snapshot.accent)
        positionEditor()
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
        send(["op":"focus", "id":node.id])
        field.text = node.value ?? ""
        field.accessibilityIdentifier = "editor-\(node.id)"
        field.accessibilityLabel = node.label
        field.isSecureTextEntry = node.password
        field.autocapitalizationType = .none
        field.keyboardType = .default
        addSubview(field)
        positionEditor()
        field.becomeFirstResponder()
        refresh()
    }
    private func positionEditor() {
        guard let id = editingId else { return }
        guard let node = nodes.first(where: { $0.id == id }) else { editingId = nil; field.resignFirstResponder(); field.removeFromSuperview(); return }
        field.frame = node.rect.insetBy(dx:2,dy:2)
        field.layer.cornerRadius = 10
        field.clipsToBounds = true
    }
    @objc private func edited() {
        guard field.markedTextRange == nil, let id = editingId else { return }
        send(["op":"text", "id":id, "text":field.text ?? ""])
    }
    @objc private func done() {
        guard let id = editingId else { return }
        // Resigning commits an active IME composition before notifying Rust.
        field.resignFirstResponder()
        let committed = field.text ?? ""
        editingId = nil
        send(["op":"text", "id":id, "text":committed])
        field.removeFromSuperview()
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
        send(["op":"begin", "x":touchStart.x,"y":touchStart.y])
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = activeTouch, touches.contains(t) else { return }
        let p = t.location(in:self)
        dragged = dragged || hypot(p.x-touchStart.x,p.y-touchStart.y) >= 8
        send(["op":"move", "x":p.x,"y":p.y])
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = activeTouch, touches.contains(t) else { return }
        let p = t.location(in:self)
        dragged = dragged || hypot(p.x-touchStart.x,p.y-touchStart.y) >= 8
        let target = !dragged ? nodes.last(where: { $0.kind == "field" && $0.enabled && !$0.readOnly && $0.visibleRect.contains(p) }) : nil
        activeTouch = nil
        if editingId != nil, !dragged,
           let button = nodes.last(where: { $0.kind == "button" && $0.enabled && $0.visibleRect.contains(p) }), button.id == touchButtonId {
            // Committing the native editor recomposes Rust and cancels its old
            // pointer capture. Treat commit + this same-control tap atomically;
            // never require a second tap on Cancel/Save while editing.
            send(["op":"cancel"])
            done()
            send(["op":"activate", "id":button.id])
            return
        }
        if target == nil { done() }
        send(["op":"end", "x":p.x,"y":p.y])
        if let target { beginEditing(target) }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        activeTouch = nil; send(["op":"cancel"])
    }
}
