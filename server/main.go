package main

import (
	"crypto/rand"
	"crypto/subtle"
	"embed"
	"encoding/hex"
	"encoding/json"
	"io"
	"io/fs"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"golang.org/x/crypto/acme/autocert"
)

/*
mango-presence: one small service behind everything MangoClient knows about
itself. It answers three different audiences from one process -

  - the mod, asking who around it is on MangoClient and what rank they hold,
  - the launcher, saying "I exist" and "I am open right now",
  - the website, showing both of those to anyone who visits.

Ports: 8880 is the API the shipped mods already point at and must never move;
80 and 443 are the same handler again, so the site is just the host name - with
a Let's Encrypt certificate of its own once MANGO_DOMAIN names one.
*/

//go:embed web
var webFiles embed.FS

var startedAt = time.Now()

var (
	store *Store
	ranks *Ranks
	admin *Admin
)

func main() {
	dir := os.Getenv("STATE_DIRECTORY")
	if dir == "" {
		dir = "."
	}
	// systemd hands us a list when there are several state directories.
	// SplitList knows the separator per platform, which a hand-rolled split on
	// ':' does not: it would cut "C:/..." in half when testing on Windows.
	if parts := filepath.SplitList(dir); len(parts) > 0 && parts[0] != "" {
		dir = parts[0]
	}

	store = NewStore(dir)
	ranks = NewRanks(dir)
	admin = NewAdmin(os.Getenv("MANGO_ADMIN_PASSWORD"))

	go store.Maintain()
	go ranks.RefreshNames()

	legacyMux := newLegacyMux()
	webMux := newWebMux()

	// 8880 is the port the mods in the wild already know; losing it would blind
	// every copy of MangoConfig ever shipped. It stays plain HTTP for exactly
	// that reason: the shipped mods ask for http://<ip>:8880 and always will.
	go listen(":8880", legacyMux)

	domain := os.Getenv("MANGO_DOMAIN")
	if domain == "" {
		// No name yet: the site is the bare IP on port 80.
		listen(":80", webMux)
		return
	}

	// With a name, Let's Encrypt issues and renews the certificate on its own,
	// cached in the state directory so a restart does not ask for a new one.
	manager := &autocert.Manager{
		Cache:      autocert.DirCache(filepath.Join(dir, "acme")),
		Prompt:     autocert.AcceptTOS,
		HostPolicy: autocert.HostWhitelist(domain, "www."+domain),
	}
	go listenTLS(":443", webMux, manager)
	// Port 80 answers the ACME challenge first, then sends anyone who came by
	// the name up to the encrypted site. Anyone who came by the bare IP is
	// served in place: there can be no certificate for an address, and the
	// site answered on that address long before it had a name.
	listen(":80", manager.HTTPHandler(upgradeByName(domain, webMux)))
}

func newLegacyMux() http.Handler {
	mux := http.NewServeMux()
	public := newIPLimiter(600, 120, 100_000)
	telemetry := newIPLimiter(120, 30, 100_000)
	mux.Handle("/v1/heartbeat", public.wrap(method(http.MethodPost, http.HandlerFunc(handleHeartbeat))))
	mux.Handle("/v1/query", public.wrap(method(http.MethodPost, http.HandlerFunc(handleQuery))))
	mux.Handle("/v1/launcher", telemetry.wrap(method(http.MethodPost, http.HandlerFunc(handleLauncher))))
	return securityHeaders(publicCORS(mux))
}

func newWebMux() http.Handler {
	mux := http.NewServeMux()
	public := newIPLimiter(600, 120, 100_000)
	mux.Handle("/v1/stats", publicCORSGet(public.wrap(method(http.MethodGet, http.HandlerFunc(handleStats)))))
	mux.Handle("/v1/ranks", publicCORSGet(public.wrap(method(http.MethodGet, http.HandlerFunc(handleRanks)))))
	mux.Handle("/v1/", http.NotFoundHandler())
	mux.Handle("/admin/login", admin.secure(method(http.MethodPost, http.HandlerFunc(admin.handleLogin))))
	mux.Handle("/admin/logout", admin.secure(admin.require(method(http.MethodPost, http.HandlerFunc(admin.handleLogout)))))
	mux.Handle("/admin/session", admin.secure(method(http.MethodGet, http.HandlerFunc(admin.handleSession))))
	mux.Handle("/admin/ranks", admin.secure(admin.require(methods([]string{http.MethodGet, http.MethodPost}, http.HandlerFunc(handleAdminRanks)))))
	mux.Handle("/", methods([]string{http.MethodGet, http.MethodHead}, http.HandlerFunc(handleSite)))
	return securityHeaders(mux)
}

func upgradeByName(domain string, plain http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		host := r.Host
		if i := strings.IndexByte(host, ':'); i >= 0 {
			host = host[:i]
		}
		if !strings.EqualFold(host, domain) && !strings.EqualFold(host, "www."+domain) {
			plain.ServeHTTP(w, r)
			return
		}
		http.Redirect(w, r, "https://"+host+r.URL.RequestURI(), http.StatusMovedPermanently)
	})
}

func listenTLS(addr string, handler http.Handler, manager *autocert.Manager) {
	server := &http.Server{
		Addr:              addr,
		Handler:           handler,
		TLSConfig:         manager.TLSConfig(),
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       20 * time.Second,
		WriteTimeout:      20 * time.Second,
		IdleTimeout:       90 * time.Second,
	}
	log.Printf("listening on %s (tls)", addr)
	log.Printf("%s: %v", addr, server.ListenAndServeTLS("", ""))
}

func listen(addr string, handler http.Handler) {
	server := &http.Server{
		Addr:              addr,
		Handler:           handler,
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       20 * time.Second,
		WriteTimeout:      20 * time.Second,
		IdleTimeout:       90 * time.Second,
	}
	log.Printf("listening on %s", addr)
	err := server.ListenAndServe()
	log.Printf("%s: %v", addr, err)
	// Port 80 may be taken or forbidden; the API on 8880 still has to live.
	if addr == ":80" {
		select {}
	}
	os.Exit(1)
}

// --- the mod ----------------------------------------------------------------

func handleHeartbeat(w http.ResponseWriter, r *http.Request) {
	var body struct {
		UUID string `json:"uuid"`
	}
	if !readJSON(w, r, &body) {
		return
	}
	store.Beat(body.UUID)
	writeJSON(w, map[string]any{"ok": true, "rank": ranks.Of(body.UUID)})
}

func handleQuery(w http.ResponseWriter, r *http.Request) {
	var body struct {
		UUIDs []string `json:"uuids"`
	}
	if !readJSON(w, r, &body) {
		return
	}
	if len(body.UUIDs) > 512 {
		body.UUIDs = body.UUIDs[:512]
	}
	online := store.Online(body.UUIDs)
	// "online" is what mods before 1.8.0 read; "ranks" is additive, so an old
	// mod keeps working and a new one gets the colours.
	writeJSON(w, map[string]any{"online": online, "ranks": ranks.Lookup(body.UUIDs)})
}

// --- the launcher -----------------------------------------------------------

func handleLauncher(w http.ResponseWriter, r *http.Request) {
	var body struct {
		ID      string `json:"id"`
		Version string `json:"version"`
		OS      string `json:"os"`
	}
	if !readJSON(w, r, &body) {
		return
	}
	store.LauncherBeat(body.ID, clip(body.Version, 24), clip(body.OS, 16))
	writeJSON(w, map[string]any{"ok": true})
}

// --- the website ------------------------------------------------------------

func handleStats(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, store.Stats())
}

func handleRanks(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]any{"ranks": ranks.All(), "order": rankOrder, "labels": rankLabels})
}

func handleAdminRanks(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodGet {
		handleRanks(w, r)
		return
	}
	var body struct {
		Action string `json:"action"`
		Rank   string `json:"rank"`
		Name   string `json:"name"`
		UUID   string `json:"uuid"`
	}
	if !readJSON(w, r, &body) {
		return
	}
	switch body.Action {
	case "add":
		member, err := ranks.Add(body.Rank, body.Name)
		if err != nil {
			fail(w, http.StatusBadRequest, err.Error())
			return
		}
		writeJSON(w, map[string]any{"ok": true, "member": member})
	case "remove":
		if err := ranks.Remove(body.Rank, body.UUID); err != nil {
			fail(w, http.StatusBadRequest, err.Error())
			return
		}
		writeJSON(w, map[string]any{"ok": true})
	default:
		fail(w, http.StatusBadRequest, "unknown action")
	}
}

// handleSite serves the embedded pages; /admin is a page like any other, it is
// the API behind it that checks the password.
func handleSite(w http.ResponseWriter, r *http.Request) {
	sub, _ := fs.Sub(webFiles, "web")
	path := strings.TrimPrefix(r.URL.Path, "/")
	switch path {
	case "":
		path = "index.html"
	case "admin":
		path = "admin.html"
	}
	data, err := fs.ReadFile(sub, path)
	if err != nil {
		http.NotFound(w, r)
		return
	}
	switch {
	case strings.HasSuffix(path, ".html"):
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
	case strings.HasSuffix(path, ".css"):
		w.Header().Set("Content-Type", "text/css; charset=utf-8")
	case strings.HasSuffix(path, ".js"):
		w.Header().Set("Content-Type", "text/javascript; charset=utf-8")
	case strings.HasSuffix(path, ".png"):
		w.Header().Set("Content-Type", "image/png")
	case strings.HasSuffix(path, ".svg"):
		w.Header().Set("Content-Type", "image/svg+xml")
	}
	w.Header().Set("Cache-Control", "no-cache")
	_, _ = w.Write(data)
}

// --- admin sessions ---------------------------------------------------------

// Admin is a password and the tokens it has handed out. No user table, no
// database: there is exactly one person who edits the staff list.
type Admin struct {
	password string
	mu       sync.Mutex
	sessions map[string]time.Time
	attempts map[string]*attempt
}

type attempt struct {
	count int
	until time.Time
}

const sessionLife = 14 * 24 * time.Hour
const adminMapCap = 10_000

func NewAdmin(password string) *Admin {
	if password == "" {
		log.Print("MANGO_ADMIN_PASSWORD is unset - the admin page cannot be used")
	}
	return &Admin{password: password, sessions: map[string]time.Time{}, attempts: map[string]*attempt{}}
}

func (a *Admin) handleLogin(w http.ResponseWriter, r *http.Request) {
	if !sameOrigin(r) {
		fail(w, http.StatusForbidden, "cross-origin request denied")
		return
	}
	var body struct {
		Password string `json:"password"`
	}
	if !readJSON(w, r, &body) {
		return
	}
	ip := clientIP(r)
	if wait, blocked := a.blocked(ip); blocked {
		fail(w, http.StatusTooManyRequests, "too many tries, wait "+wait.Truncate(time.Second).String())
		return
	}
	if a.password == "" || subtle.ConstantTimeCompare([]byte(body.Password), []byte(a.password)) != 1 {
		a.miss(ip)
		fail(w, http.StatusUnauthorized, "wrong password")
		return
	}

	token := randomToken()
	a.mu.Lock()
	a.pruneLocked(time.Now())
	if len(a.sessions) >= adminMapCap {
		a.mu.Unlock()
		fail(w, http.StatusServiceUnavailable, "too many admin sessions")
		return
	}
	a.sessions[token] = time.Now().Add(sessionLife)
	delete(a.attempts, ip)
	for old, expiry := range a.sessions { // sweep, so tokens cannot pile up forever
		if time.Now().After(expiry) {
			delete(a.sessions, old)
		}
	}
	a.mu.Unlock()

	http.SetCookie(w, &http.Cookie{
		Name:     "mango_admin",
		Value:    token,
		Path:     "/",
		HttpOnly: true,
		Secure:   true,
		SameSite: http.SameSiteStrictMode,
		MaxAge:   int(sessionLife.Seconds()),
	})
	writeJSON(w, map[string]any{"ok": true})
}

func (a *Admin) handleLogout(w http.ResponseWriter, r *http.Request) {
	if !sameOrigin(r) {
		fail(w, http.StatusForbidden, "cross-origin request denied")
		return
	}
	if cookie, err := r.Cookie("mango_admin"); err == nil {
		a.mu.Lock()
		delete(a.sessions, cookie.Value)
		a.mu.Unlock()
	}
	http.SetCookie(w, &http.Cookie{Name: "mango_admin", Value: "", Path: "/", MaxAge: -1, HttpOnly: true, Secure: true, SameSite: http.SameSiteStrictMode})
	writeJSON(w, map[string]any{"ok": true})
}

func (a *Admin) handleSession(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]any{"authed": a.authed(r), "configured": a.password != ""})
}

func (a *Admin) require(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !a.authed(r) {
			fail(w, http.StatusUnauthorized, "sign in first")
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (a *Admin) secure(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if a.password == "" || r.TLS == nil {
			fail(w, http.StatusForbidden, "admin requires configured HTTPS")
			return
		}
		if r.Method == http.MethodPost && !sameOrigin(r) {
			fail(w, http.StatusForbidden, "cross-origin request denied")
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (a *Admin) authed(r *http.Request) bool {
	cookie, err := r.Cookie("mango_admin")
	if err != nil {
		return false
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	expiry, ok := a.sessions[cookie.Value]
	if !ok || time.Now().After(expiry) {
		delete(a.sessions, cookie.Value)
		return false
	}
	return true
}

func (a *Admin) blocked(ip string) (time.Duration, bool) {
	a.mu.Lock()
	defer a.mu.Unlock()
	rec, ok := a.attempts[ip]
	if !ok || time.Now().After(rec.until) {
		return 0, false
	}
	return time.Until(rec.until), rec.count >= 5
}

// miss slows a guesser down: five tries, then a minute more per try after that.
func (a *Admin) miss(ip string) {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.pruneLocked(time.Now())
	rec, ok := a.attempts[ip]
	if !ok && len(a.attempts) >= adminMapCap {
		return
	}
	if !ok || time.Now().After(rec.until) {
		rec = &attempt{}
		a.attempts[ip] = rec
	}
	rec.count++
	rec.until = time.Now().Add(time.Duration(rec.count) * time.Minute)
}

func (a *Admin) pruneLocked(now time.Time) {
	for token, expiry := range a.sessions {
		if !expiry.After(now) {
			delete(a.sessions, token)
		}
	}
	for ip, rec := range a.attempts {
		if !rec.until.After(now) {
			delete(a.attempts, ip)
		}
	}
}

func randomToken() string {
	buf := make([]byte, 32)
	_, _ = rand.Read(buf)
	return hex.EncodeToString(buf)
}

// --- plumbing ---------------------------------------------------------------

func readJSON(w http.ResponseWriter, r *http.Request, into any) bool {
	r.Body = http.MaxBytesReader(w, r.Body, 64<<10)
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(into); err != nil {
		fail(w, http.StatusBadRequest, "bad json")
		return false
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		fail(w, http.StatusBadRequest, "one json value only")
		return false
	}
	return true
}

func writeJSON(w http.ResponseWriter, value any) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(value)
}

func fail(w http.ResponseWriter, status int, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]any{"error": message})
}

func clip(s string, n int) string {
	s = strings.TrimSpace(s)
	if len(s) > n {
		return s[:n]
	}
	return s
}

func clientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}

func method(want string, next http.Handler) http.Handler { return methods([]string{want}, next) }

func methods(allowed []string, next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		for _, candidate := range allowed {
			if r.Method == candidate {
				next.ServeHTTP(w, r)
				return
			}
		}
		w.Header().Set("Allow", strings.Join(allowed, ", "))
		fail(w, http.StatusMethodNotAllowed, "method not allowed")
	})
}

func sameOrigin(r *http.Request) bool {
	origin := r.Header.Get("Origin")
	if origin == "" {
		return false
	}
	want := "https://" + r.Host
	return subtle.ConstantTimeCompare([]byte(strings.ToLower(origin)), []byte(strings.ToLower(want))) == 1
}

func securityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Security-Policy", "default-src 'self'; img-src 'self' data:; script-src 'self'; style-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("X-Frame-Options", "DENY")
		w.Header().Set("Referrer-Policy", "no-referrer")
		w.Header().Set("Permissions-Policy", "camera=(), microphone=(), geolocation=()")
		if r.TLS != nil {
			w.Header().Set("Strict-Transport-Security", "max-age=31536000; includeSubDomains")
		}
		next.ServeHTTP(w, r)
	})
}

func publicCORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
		w.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func publicCORSGet(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, OPTIONS")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

type rateRecord struct {
	tokens     float64
	last, seen time.Time
}
type ipLimiter struct {
	mu          sync.Mutex
	rate, burst float64
	cap         int
	entries     map[string]*rateRecord
}

func newIPLimiter(perMinute, burst, cap int) *ipLimiter {
	return &ipLimiter{rate: float64(perMinute) / 60, burst: float64(burst), cap: cap, entries: map[string]*rateRecord{}}
}

func (l *ipLimiter) allow(ip string, now time.Time) bool {
	l.mu.Lock()
	defer l.mu.Unlock()
	rec := l.entries[ip]
	if rec == nil {
		if len(l.entries) >= l.cap {
			for key, old := range l.entries {
				if now.Sub(old.seen) > 10*time.Minute {
					delete(l.entries, key)
				}
			}
			if len(l.entries) >= l.cap {
				return false
			}
		}
		rec = &rateRecord{tokens: l.burst, last: now}
		l.entries[ip] = rec
	}
	rec.tokens = min(l.burst, rec.tokens+now.Sub(rec.last).Seconds()*l.rate)
	rec.last, rec.seen = now, now
	if rec.tokens < 1 {
		return false
	}
	rec.tokens--
	return true
}

func (l *ipLimiter) wrap(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !l.allow(clientIP(r), time.Now()) {
			fail(w, http.StatusTooManyRequests, "rate limit exceeded")
			return
		}
		next.ServeHTTP(w, r)
	})
}
