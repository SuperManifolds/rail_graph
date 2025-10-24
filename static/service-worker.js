const CACHE_VERSION = 'railgraph-v1';
const CACHE_NAME = `${CACHE_VERSION}-app`;

// Font Awesome CSS to cache
const FONT_AWESOME_URL = 'https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css';

// Install event - cache app shell and assets from manifest
self.addEventListener('install', event => {
    console.log('[SW] Installing service worker...');

    event.waitUntil(
        fetch('/asset-manifest.json')
            .then(response => response.json())
            .then(manifest => {
                console.log('[SW] Loading asset manifest, version:', manifest.version);
                console.log('[SW] Assets to cache:', manifest.assets.length);

                return caches.open(CACHE_NAME).then(cache => {
                    // Cache Font Awesome CSS
                    const fontAwesomePromise = cache.add(FONT_AWESOME_URL).catch(err => {
                        console.warn('[SW] Failed to cache Font Awesome:', err);
                    });

                    // Cache all assets from manifest
                    const assetPromises = manifest.assets.map(url =>
                        cache.add(url).catch(err => {
                            console.warn('[SW] Failed to cache asset:', url, err);
                        })
                    );

                    return Promise.all([fontAwesomePromise, ...assetPromises]);
                });
            })
            .then(() => {
                console.log('[SW] Install complete');
                return self.skipWaiting();
            })
            .catch(err => {
                console.error('[SW] Install failed:', err);
            })
    );
});

// Activate event - clean up old caches
self.addEventListener('activate', event => {
    console.log('[SW] Activating service worker...');

    event.waitUntil(
        caches.keys()
            .then(cacheNames => {
                return Promise.all(
                    cacheNames
                        .filter(name => name.startsWith('railgraph-') && name !== CACHE_NAME)
                        .map(name => {
                            console.log('[SW] Deleting old cache:', name);
                            return caches.delete(name);
                        })
                );
            })
            .then(() => {
                console.log('[SW] Activation complete');
                return self.clients.claim();
            })
    );
});

// Fetch event - serve from cache, fallback to network
self.addEventListener('fetch', event => {
    const url = new URL(event.request.url);

    // Skip non-GET requests
    if (event.request.method !== 'GET') {
        return;
    }

    // Network-first for API calls
    if (url.pathname.startsWith('/api/')) {
        event.respondWith(
            fetch(event.request)
                .catch(err => {
                    console.error('[SW] API request failed:', err);
                    throw err;
                })
        );
        return;
    }

    // Cache-first for everything else
    event.respondWith(
        caches.match(event.request)
            .then(cachedResponse => {
                if (cachedResponse) {
                    return cachedResponse;
                }

                // Not in cache, fetch from network
                return fetch(event.request)
                    .then(response => {
                        // Don't cache if not a successful response
                        if (!response || response.status !== 200 || response.type === 'error') {
                            return response;
                        }

                        // Clone the response
                        const responseToCache = response.clone();

                        // Cache the fetched resource
                        caches.open(CACHE_NAME)
                            .then(cache => {
                                cache.put(event.request, responseToCache);
                            });

                        return response;
                    })
                    .catch(err => {
                        console.error('[SW] Fetch failed:', event.request.url, err);
                        throw err;
                    });
            })
    );
});
