// Register service worker for PWA functionality
if ('serviceWorker' in navigator) {
    window.addEventListener('load', () => {
        navigator.serviceWorker.register('/static/service-worker.js')
            .then(registration => {
                console.log('[PWA] Service worker registered:', registration.scope);
            })
            .catch(error => {
                console.error('[PWA] Service worker registration failed:', error);
            });
    });
}
