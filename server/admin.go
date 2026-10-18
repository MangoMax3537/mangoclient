package main

import (
	"net/http"
	"runtime"
	"strconv"
	"time"
)

/*
The dashboard's API. Every route here sits behind admin.secure and
admin.require, so it only ever answers a signed-in browser over HTTPS.
*/

func handleAdminOverview(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, store.Overview())
}

func handleAdminLive(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]any{"players": store.Live(), "at": time.Now()})
}

func handleAdminPlayers(w http.ResponseWriter, r *http.Request) {
	limit, _ := strconv.Atoi(r.URL.Query().Get("limit"))
	if limit <= 0 || limit > 2000 {
		limit = 500
	}
	players, total := store.Directory(clip(r.URL.Query().Get("q"), 64), limit)
	writeJSON(w, map[string]any{"players": players, "total": total})
}

func handleAdminInstalls(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]any{"installs": store.Installs()})
}

func handleAdminSystem(w http.ResponseWriter, r *http.Request) {
	var mem runtime.MemStats
	runtime.ReadMemStats(&mem)
	admin.mu.Lock()
	sessions := len(admin.sessions)
	admin.mu.Unlock()
	writeJSON(w, map[string]any{
		"startedAt":     startedAt,
		"uptime":        int64(time.Since(startedAt).Seconds()),
		"go":            runtime.Version(),
		"goroutines":    runtime.NumGoroutine(),
		"heapMB":        float64(mem.HeapAlloc) / (1 << 20),
		"sysMB":         float64(mem.Sys) / (1 << 20),
		"gcRuns":        mem.NumGC,
		"adminSessions": sessions,
		"domain":        r.Host,
	})
}
