package main

import (
	"crypto/tls"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func setupHandlers(t *testing.T, password string) (http.Handler, http.Handler) {
	t.Helper()
	dir := t.TempDir()
	store = NewStore(dir)
	ranks = NewRanks(dir)
	admin = NewAdmin(password)
	return newLegacyMux(), newWebMux()
}

func request(handler http.Handler, method, target, body string, tlsOn bool) *httptest.ResponseRecorder {
	r := httptest.NewRequest(method, target, strings.NewReader(body))
	r.RemoteAddr = "192.0.2.1:1234"
	if tlsOn {
		r.TLS = &tls.ConnectionState{}
	}
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, r)
	return w
}

func TestLegacyAndWebRoutesAreSeparated(t *testing.T) {
	legacy, web := setupHandlers(t, "secret")
	if got := request(legacy, http.MethodGet, "/", "", false).Code; got != http.StatusNotFound {
		t.Fatalf("legacy site status = %d", got)
	}
	if got := request(legacy, http.MethodGet, "/admin/session", "", false).Code; got != http.StatusNotFound {
		t.Fatalf("legacy admin status = %d", got)
	}
	if got := request(web, http.MethodPost, "/v1/heartbeat", `{}`, true).Code; got != http.StatusNotFound {
		t.Fatalf("web presence status = %d", got)
	}
	if got := request(legacy, http.MethodGet, "/v1/query", "", false).Code; got != http.StatusMethodNotAllowed {
		t.Fatalf("method status = %d", got)
	}
}

func TestAdminRequiresTLSOriginAndSecureCookie(t *testing.T) {
	_, web := setupHandlers(t, "secret")
	if got := request(web, http.MethodPost, "/admin/login", `{"password":"secret"}`, false).Code; got != http.StatusForbidden {
		t.Fatalf("plaintext login status = %d", got)
	}
	r := httptest.NewRequest(http.MethodPost, "https://mango.example/admin/login", strings.NewReader(`{"password":"secret"}`))
	r.TLS = &tls.ConnectionState{}
	r.RemoteAddr = "192.0.2.1:1234"
	r.Header.Set("Origin", "https://mango.example")
	w := httptest.NewRecorder()
	web.ServeHTTP(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("secure login status = %d, body %s", w.Code, w.Body.String())
	}
	cookies := w.Result().Cookies()
	if len(cookies) != 1 || !cookies[0].Secure || !cookies[0].HttpOnly || cookies[0].SameSite != http.SameSiteStrictMode {
		t.Fatalf("insecure cookie: %#v", cookies)
	}
	r.Header.Set("Origin", "https://evil.example")
	w = httptest.NewRecorder()
	web.ServeHTTP(w, r)
	if w.Code != http.StatusForbidden {
		t.Fatalf("cross-origin status = %d", w.Code)
	}
}

func TestBodyLimitCorsAndSecurityHeaders(t *testing.T) {
	legacy, web := setupHandlers(t, "secret")
	huge := `{"uuid":"` + strings.Repeat("a", 70<<10) + `"}`
	w := request(legacy, http.MethodPost, "/v1/heartbeat", huge, false)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("large body status = %d", w.Code)
	}
	if w.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Fatal("public legacy CORS missing")
	}
	if w.Header().Get("X-Content-Type-Options") != "nosniff" {
		t.Fatal("security headers missing")
	}
	w = request(web, http.MethodGet, "/v1/stats", "", true)
	if w.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Fatal("public web CORS missing")
	}
	if w.Header().Get("Strict-Transport-Security") == "" {
		t.Fatal("HSTS missing")
	}
	w = request(web, http.MethodGet, "/admin/session", "", true)
	if w.Header().Get("Access-Control-Allow-Origin") != "" {
		t.Fatal("admin CORS must be absent")
	}
}

func TestRateLimitAndBoundedAdminMaps(t *testing.T) {
	limiter := newIPLimiter(60, 2, 2)
	now := time.Now()
	if !limiter.allow("a", now) || !limiter.allow("a", now) || limiter.allow("a", now) {
		t.Fatal("burst was not enforced")
	}
	a := NewAdmin("secret")
	for i := 0; i < adminMapCap+50; i++ {
		a.miss(fmt.Sprintf("192.0.2.%d", i))
	}
	if len(a.attempts) > adminMapCap {
		t.Fatalf("attempt map grew to %d", len(a.attempts))
	}
	before := a.attempts["192.0.2.0"].count
	a.miss("192.0.2.0")
	if a.attempts["192.0.2.0"].count != before+1 {
		t.Fatal("existing rate record stopped updating at the cap")
	}
	a.sessions["expired"] = now.Add(-time.Second)
	a.attempts["expired"] = &attempt{until: now.Add(-time.Second)}
	a.mu.Lock()
	a.pruneLocked(now)
	a.mu.Unlock()
	if _, ok := a.sessions["expired"]; ok {
		t.Fatal("expired session was not pruned")
	}
	if _, ok := a.attempts["expired"]; ok {
		t.Fatal("expired attempt was not pruned")
	}
}

func TestStoreCapsPresenceAndSeenPlayers(t *testing.T) {
	s := NewStore(t.TempDir())
	now := time.Now()
	for i := 0; i < maxActivePlayers; i++ {
		s.players[fmt.Sprintf("%032x", i)] = now
	}
	s.Beat("ffffffffffffffffffffffffffffffff")
	if len(s.players) != maxActivePlayers {
		t.Fatalf("players grew to %d", len(s.players))
	}
	s.players = map[string]time.Time{}
	for i := 0; i < maxSeenPlayers; i++ {
		s.seen[fmt.Sprintf("%032x", i)] = now
	}
	s.Beat("ffffffffffffffffffffffffffffffff")
	if len(s.seen) != maxSeenPlayers {
		t.Fatalf("seen grew to %d", len(s.seen))
	}
}
