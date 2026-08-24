package main

import (
	"encoding/json"
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

/*
The whole of MangoClient's memory: who is playing, who has the launcher open,
and how many people ever installed it.

Presence is deliberately in-memory only - it is a question about *now*, and a
restart that forgets it is repaired within one heartbeat. Installs are the
opposite: the count only means something if it survives reboots, so they are
written to disk (lazily, from one goroutine, never on the request path).
*/

// How long a heartbeat vouches for someone. Clients beat every 30 s, so this
// tolerates two lost beats before a player blinks out.
const onlineTTL = 3 * time.Minute

// An install is "active" if the launcher was opened in the last month; that is
// the honest number to put next to a lifetime download count.
const activeWindow = 30 * 24 * time.Hour
const maxActivePlayers = 100_000
const maxSeenPlayers = 1_000_000

// Install is one copy of the launcher on one machine, keyed by a random id the
// launcher generates on first run. No account, no hardware id, nothing that
// points back at a person.
type Install struct {
	First    time.Time `json:"first"`
	Last     time.Time `json:"last"`
	Version  string    `json:"version,omitempty"`
	OS       string    `json:"os,omitempty"`
	Sessions int       `json:"sessions,omitempty"`
}

type Store struct {
	mu sync.Mutex

	// uuid (normalised) -> last heartbeat, for players in-game with the mod.
	players map[string]time.Time
	// install id -> record, for launchers that are open right now and ever.
	installs map[string]*Install
	// uuid -> last seen, so "players ever" is a real number and not a guess.
	seen map[string]time.Time

	dir   string
	dirty bool
}

func NewStore(dir string) *Store {
	s := &Store{
		players:  map[string]time.Time{},
		installs: map[string]*Install{},
		seen:     map[string]time.Time{},
		dir:      dir,
	}
	s.load()
	return s
}

// --- presence ---------------------------------------------------------------

// Beat records that a player is in-game on MangoClient right now.
func (s *Store) Beat(uuid string) {
	key := normaliseUUID(uuid)
	if key == "" {
		return
	}
	now := time.Now()
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, exists := s.players[key]; !exists && len(s.players) >= maxActivePlayers {
		s.sweepLocked(now)
		if len(s.players) >= maxActivePlayers {
			return
		}
	}
	s.players[key] = now
	if _, ok := s.seen[key]; !ok && len(s.seen) < maxSeenPlayers {
		s.dirty = true
		s.seen[key] = now
	} else if ok {
		s.seen[key] = now
	}
}

// Online filters the caller's list down to the ones we have heard from,
// answering with the exact strings it asked with so no client has to care
// about how we spell a uuid.
func (s *Store) Online(uuids []string) []string {
	cutoff := time.Now().Add(-onlineTTL)
	out := make([]string, 0, len(uuids))
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, raw := range uuids {
		if last, ok := s.players[normaliseUUID(raw)]; ok && last.After(cutoff) {
			out = append(out, raw)
		}
	}
	return out
}

// LauncherBeat records that a launcher is open, and registers it as an install
// the first time we ever see its id.
func (s *Store) LauncherBeat(id, version, osName string) {
	if len(id) < 8 || len(id) > 64 {
		return
	}
	now := time.Now()
	s.mu.Lock()
	defer s.mu.Unlock()
	rec, ok := s.installs[id]
	if !ok {
		if len(s.installs) > 500_000 {
			return
		}
		rec = &Install{First: now}
		s.installs[id] = rec
	}
	// A gap longer than the online window means this is a fresh open, not the
	// same session still beating.
	if now.Sub(rec.Last) > onlineTTL {
		rec.Sessions++
	}
	rec.Last = now
	rec.Version = version
	rec.OS = osName
	s.dirty = true
}

// Stats is the shape the website and /v1/stats speak.
type Stats struct {
	InGame        int            `json:"inGame"`
	LaunchersOpen int            `json:"launchersOpen"`
	Installs      int            `json:"installs"`
	ActiveMonth   int            `json:"activeMonth"`
	PlayersEver   int            `json:"playersEver"`
	Sessions      int            `json:"sessions"`
	ByOS          map[string]int `json:"byOS"`
	ByVersion     map[string]int `json:"byVersion"`
	Since         time.Time      `json:"since"`
}

func (s *Store) Stats() Stats {
	now := time.Now()
	cutoff := now.Add(-onlineTTL)
	active := now.Add(-activeWindow)

	st := Stats{ByOS: map[string]int{}, ByVersion: map[string]int{}, Since: startedAt}
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, last := range s.players {
		if last.After(cutoff) {
			st.InGame++
		}
	}
	for _, rec := range s.installs {
		st.Installs++
		st.Sessions += rec.Sessions
		if rec.Last.After(cutoff) {
			st.LaunchersOpen++
		}
		if rec.Last.After(active) {
			st.ActiveMonth++
			if rec.OS != "" {
				st.ByOS[rec.OS]++
			}
			if rec.Version != "" {
				st.ByVersion[rec.Version]++
			}
		}
	}
	st.PlayersEver = len(s.seen)
	return st
}

// --- housekeeping and disk --------------------------------------------------

// Maintain drops stale presence and flushes what changed. One goroutine owns
// both jobs so nothing on the request path ever waits on a disk write.
func (s *Store) Maintain() {
	for range time.Tick(30 * time.Second) {
		s.mu.Lock()
		s.sweepLocked(time.Now())
		dirty := s.dirty
		s.dirty = false
		var installs, seen []byte
		if dirty {
			installs, _ = json.Marshal(s.installs)
			seen, _ = json.Marshal(s.seen)
		}
		s.mu.Unlock()
		if dirty {
			writeAtomic(filepath.Join(s.dir, "installs.json"), installs)
			writeAtomic(filepath.Join(s.dir, "players.json"), seen)
		}
	}
}

func (s *Store) sweepLocked(now time.Time) {
	cutoff := now.Add(-onlineTTL)
	for key, last := range s.players {
		if last.Before(cutoff) {
			delete(s.players, key)
		}
	}
}

func (s *Store) load() {
	if data, err := os.ReadFile(filepath.Join(s.dir, "installs.json")); err == nil {
		_ = json.Unmarshal(data, &s.installs)
	}
	if data, err := os.ReadFile(filepath.Join(s.dir, "players.json")); err == nil {
		_ = json.Unmarshal(data, &s.seen)
	}
	if len(s.seen) > maxSeenPlayers {
		bounded := make(map[string]time.Time, maxSeenPlayers)
		for key, seen := range s.seen {
			bounded[key] = seen
			if len(bounded) == maxSeenPlayers {
				break
			}
		}
		s.seen = bounded
	}
}

func writeAtomic(path string, data []byte) {
	if len(data) == 0 {
		return
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		log.Printf("cannot write %s: %v", tmp, err)
		return
	}
	if err := os.Rename(tmp, path); err != nil {
		log.Printf("cannot replace %s: %v", path, err)
	}
	_ = os.Chmod(path, 0o600)
}

// normaliseUUID makes "0a1b..." and "0A1B-..." the same key, and rejects
// anything that is not a uuid at all.
func normaliseUUID(s string) string {
	s = strings.ToLower(strings.TrimSpace(s))
	s = strings.ReplaceAll(s, "-", "")
	if len(s) != 32 {
		return ""
	}
	for i := 0; i < len(s); i++ {
		c := s[i]
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'f') {
			return ""
		}
	}
	return s
}
