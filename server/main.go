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

	mux := http.NewServeMux()

	// --- the mod -------------------------------------------------------------
	mux.HandleFunc("/v1/heartbeat", handleHeartbeat)
	mux.HandleFunc("/v1/query", handleQuery)

	// --- the launcher --------------------------------------------------------
	mux.HandleFunc("/v1/launcher", handleLauncher)

	// --- the website ---------------------------------------------------------
	mux.HandleFunc("/v1/stats", handleStats)
	mux.HandleFunc("/v1/ranks", handleRanks)
	mux.HandleFunc("/admin/login", admin.handleLogin)
	mux.HandleFunc("/admin/logout", admin.handleLogout)
	mux.HandleFunc("/admin/session", admin.handleSession)
	mux.HandleFunc("/admin/ranks", admin.require(handleAdminRanks))
	mux.HandleFunc("/", handleSite)

	// 8880 is the port the mods in the wild already know; losing it would blind
	// every copy of MangoConfig ever shipped. It stays plain HTTP for exactly
	// that reason: the shipped mods ask for http://<ip>:8880 and always will.
	go listen(":8880", mux)

	domain := os.Getenv("MANGO_DOMAIN")
	if domain == "" {
		// No name yet: the site is the bare IP on port 80.
		listen(":80", mux)
		return
	}

	// With a name, Let's Encrypt issues and renews the certificate on its own,
	// cached in the state directory so a restart does not ask for a new one.
	manager := &autocert.Manager{
		Cache:      autocert.DirCache(filepath.Join(dir, "acme")),
		Prompt:     autocert.AcceptTOS,
		HostPolicy: autocert.HostWhitelist(domain, "www."+domain),
	}
	go listenTLS(":443", mux, manager)
	// Port 80 answers the ACME challenge first, then sends anyone who came by
	// the name up to the encrypted site. Anyone who came by the bare IP is
	// served in place: there can be no certificate for an address, and the
	// site answered on that address long before it had a name.
	listen(":80", manager.HTTPHandler(upgradeByName(domain, mux)))
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

func NewAdmin(password string) *Admin {
	if password == "" {
		log.Print("MANGO_ADMIN_PASSWORD is unset - the admin page cannot be used")
	}
	return &Admin{password: password, sessions: map[string]time.Time{}, attempts: map[string]*attempt{}}
}

func (a *Admin) handleLogin(w http.ResponseWriter, r *http.Request) {
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
		SameSite: http.SameSiteLaxMode,
		MaxAge:   int(sessionLife.Seconds()),
	})
	writeJSON(w, map[string]any{"ok": true})
}

func (a *Admin) handleLogout(w http.ResponseWriter, r *http.Request) {
	if cookie, err := r.Cookie("mango_admin"); err == nil {
		a.mu.Lock()
		delete(a.sessions, cookie.Value)
		a.mu.Unlock()
	}
	http.SetCookie(w, &http.Cookie{Name: "mango_admin", Value: "", Path: "/", MaxAge: -1})
	writeJSON(w, map[string]any{"ok": true})
}

func (a *Admin) handleSession(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]any{"authed": a.authed(r), "configured": a.password != ""})
}

func (a *Admin) require(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if !a.authed(r) {
			fail(w, http.StatusUnauthorized, "sign in first")
			return
		}
		next(w, r)
	}
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
	rec, ok := a.attempts[ip]
	if !ok || time.Now().After(rec.until) {
		rec = &attempt{}
		a.attempts[ip] = rec
	}
	rec.count++
	rec.until = time.Now().Add(time.Duration(rec.count) * time.Minute)
}

func randomToken() string {
	buf := make([]byte, 32)
	_, _ = rand.Read(buf)
	return hex.EncodeToString(buf)
}

// --- plumbing ---------------------------------------------------------------

func readJSON(w http.ResponseWriter, r *http.Request, into any) bool {
	if r.Method != http.MethodPost {
		fail(w, http.StatusMethodNotAllowed, "POST only")
		return false
	}
	data, err := io.ReadAll(io.LimitReader(r.Body, 1<<18))
	if err != nil {
		fail(w, http.StatusBadRequest, "unreadable body")
		return false
	}
	if err := json.Unmarshal(data, into); err != nil {
		fail(w, http.StatusBadRequest, "bad json")
		return false
	}
	return true
}

func writeJSON(w http.ResponseWriter, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Access-Control-Allow-Origin", "*")
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
