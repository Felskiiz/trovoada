import SwiftUI
import WebKit
import CoreLocation

@main
struct TrovoadaApp: App {
    var body: some Scene {
        WindowGroup {
            Radar()
                .ignoresSafeArea()
                .background(Color(red: 0.04, green: 0.04, blue: 0.04))
                .preferredColorScheme(.dark)
        }
    }
}

// O site público com a paleta preto e branco (?pb); o servidor Rust não roda no iPhone.
struct Radar: UIViewRepresentable {
    func makeUIView(context: Context) -> WKWebView {
        context.coordinator.gps.requestWhenInUseAuthorization()
        let web = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())
        web.isOpaque = false
        web.backgroundColor = .black
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.scrollView.bounces = false
        web.uiDelegate = context.coordinator
        web.load(URLRequest(url: URL(string: "https://felskiiz.github.io/trovoada/?pb")!))
        return web
    }

    func updateUIView(_ web: WKWebView, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator: NSObject, WKUIDelegate {
        let gps = CLLocationManager()

        // Sem isto o WKWebView pergunta a localização de novo a cada abertura.
        @available(iOS 27.0, *)
        func webView(_ webView: WKWebView, requestGeolocationPermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo, decisionHandler: @escaping @MainActor (WKPermissionDecision) -> Void) {
            decisionHandler(.grant)
        }
    }
}
