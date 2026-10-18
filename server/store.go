package main

import (
	"encoding/json"
	"log"
	"os"
	"path/filepath"
	"sort"
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

The admin dashboard adds three things on top, all written the same lazy way:
a profile per player (name, last server, playtime), a five-minute history of
the two live numbers, and per-day sets of who showed up.
*/

// How long a heartbeat vouches for someone. Clients beat every 30 s, so this
// tolerates two lost beats before a player blinks out.
const onlineTTL = 3 * time.Minute

// An install is "active" if the launcher was opened in the last month; that is
// the honest number to put next to a lifetime download count.
const activeWindow = 30 * 24 * time.Hour
const maxActivePlayers = 100_000
const maxSeenPlayers = 1_000_000

// The dashboard's memory: a week of five-minute samples, two months of days.
const sampleEvery = 5 * time.Minute
const maxSamples = 7 * 24 * 12
const keepDays = 60

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

// BeatInfo is what MangoConfig 1.10.1+ says about itself next to the uuid.
// Older mods send none of it, and every field may be empty.
type BeatInfo struct {
	Name   string
	MC     string
	Mod    string
	Server string
}

// Session is one player in game right now.
type Session struct {
	BeatInfo
	Since time.Time
	Last  time.Time
}

// Profile is everything we remember about a player between sessions.
type Profile struct {
	Name     string    `json:"name,omitempty"`
	First    time.Time `json:"first"`
	Last     time.Time `json:"last"`
	MC       string    `json:"mc,omitempty"`
	Mod      string    `json:"mod,omitempty"`
	Server   string    `json:"server,omitempty"`
	Sessions int       `json:"sessions,omitempty"`
	Playtime int64     `json:"playtime,omitempty"` // seconds
}

// Sample is one point on the dashboard's live chart.
type Sample struct {
	T         time.Time `json:"t"`
	InGame    int       `json:"inGame"`
	Launchers int       `json:"launchers"`
}

// Day is one calendar day (UTC): who played, which launchers opened, the peak.
type Day struct {
	Players   map[string]bool `json:"players"`
	Launchers map[string]bool `json:"launchers"`
	New       int             `json:"new"`
	Peak      int             `json:"peak"`
}

type Store struct {
	mu sync.Mutex

	// uuid (normalised) -> session, for players in-game with the mod.
	players map[string]*Session
	// install id -> record, for launchers that are open right now and ever.
	installs map[string]*Install
	// uuid -> last seen, so "players ever" is a real number and not a guess.
	seen map[string]time.Time
	// uuid -> what we know about them, for the admin directory.
	profiles map[string]*Profile
	history  []Sample
	days     map[string]*Day

	// uuids whose name nobody told us; one goroutine asks Mojang, slowly.
	unnamed chan string

	dir   string
	dirty bool
}

func NewStore(dir string) *Store {
	s := &Store{
		players:  map[string]*Session{},
		installs: map[string]*Install{},
		seen:     map[string]time.Time{},
		profiles: map[string]*Profile{},
		days:     map[string]*Day{},
		unnamed:  make(chan string, 1024),
		dir:      dir,
	}
	s.load()
	return s
}

// --- presence ---------------------------------------------------------------

// Beat records that a player is in-game on MangoClient right now.
func (s *Store) Beat(uuid string, info BeatInfo) {
	key := normaliseUUID(uuid)
	if key == "" {
		return
	}
	info = cleanBeat(info)
	now := time.Now()
	s.mu.Lock()
	defer s.mu.Unlock()
	session, exists := s.players[key]
	if !exists && len(s.players) >= maxActivePlayers {
		s.sweepLocked(now)
		if len(s.players) >= maxActivePlayers {
			return
		}
	}
	if _, ok := s.seen[key]; !ok && len(s.seen) < maxSeenPlayers {
		s.dirty = true
		s.seen[key] = now
	} else if ok {
		s.seen[key] = now
	}

	profile := s.profiles[key]
	if profile == nil {
		if len(s.profiles) >= maxSeenPlayers {
			profile = &Profile{} // counted nowhere, but never nil below
		} else {
			profile = &Profile{First: now}
			s.profiles[key] = profile
		}
	}

	fresh := !exists || now.Sub(session.Last) > onlineTTL
	if fresh {
		session = &Session{Since: now}
		s.players[key] = session
		profile.Sessions++
	} else {
		profile.Playtime += int64(now.Sub(session.Last).Seconds())
	}
	session.Last = now

	// An old mod sends only the uuid: keep what a newer beat or Mojang told us.
	if info.Name != "" {
		session.Name, profile.Name = info.Name, info.Name
	} else if profile.Name != "" {
		session.Name = profile.Name
	}
	if info.MC != "" {
		session.MC, profile.MC = info.MC, info.MC
	}
	if info.Mod != "" {
		session.Mod, profile.Mod = info.Mod, info.Mod
	}
	session.Server = info.Server
	if info.Server != "" {
		profile.Server = info.Server
	}
	profile.Last = now
	s.dirty = true

	day := s.dayLocked(now)
	day.Players[key] = true

	if session.Name == "" && fresh {
		select {
		case s.unnamed <- key:
		default:
		}
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
		if session, ok := s.players[normaliseUUID(raw)]; ok && session.Last.After(cutoff) {
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
	day := s.dayLocked(now)
	rec, ok := s.installs[id]
	if !ok {
		if len(s.installs) > 500_000 {
			return
		}
		rec = &Install{First: now}
		s.installs[id] = rec
		day.New++
	}
	// A gap longer than the online window means this is a fresh open, not the
	// same session still beating.
	if now.Sub(rec.Last) > onlineTTL {
		rec.Sessions++
	}
	rec.Last = now
	rec.Version = version
	rec.OS = osName
	day.Launchers[id] = true
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
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.statsLocked(time.Now())
}

func (s *Store) statsLocked(now time.Time) Stats {
	cutoff := now.Add(-onlineTTL)
	active := now.Add(-activeWindow)

	st := Stats{ByOS: map[string]int{}, ByVersion: map[string]int{}, Since: startedAt}
	for _, session := range s.players {
		if session.Last.After(cutoff) {
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

// --- the admin's view -------------------------------------------------------

// LivePlayer is one row of the admin's "online now" table.
type LivePlayer struct {
	UUID   string    `json:"uuid"`
	Name   string    `json:"name"`
	MC     string    `json:"mc"`
	Mod    string    `json:"mod"`
	Server string    `json:"server"`
	Since  time.Time `json:"since"`
	Last   time.Time `json:"last"`
	Rank   string    `json:"rank"`
}

func (s *Store) Live() []LivePlayer {
	cutoff := time.Now().Add(-onlineTTL)
	s.mu.Lock()
	out := make([]LivePlayer, 0, len(s.players))
	for key, session := range s.players {
		if !session.Last.After(cutoff) {
			continue
		}
		out = append(out, LivePlayer{
			UUID: key, Name: session.Name, MC: session.MC, Mod: session.Mod,
			Server: session.Server, Since: session.Since, Last: session.Last,
		})
	}
	s.mu.Unlock()
	for i := range out {
		out[i].Rank = ranks.Of(out[i].UUID)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Since.Before(out[j].Since) })
	return out
}

// KnownPlayer is one row of the directory of everyone ever seen.
type KnownPlayer struct {
	UUID string `json:"uuid"`
	Profile
	Online bool   `json:"online"`
	Rank   string `json:"rank"`
}

// Directory lists everyone ever seen, most recent first, optionally filtered
// by a piece of their name, uuid or last server.
func (s *Store) Directory(query string, limit int) ([]KnownPlayer, int) {
	query = strings.ToLower(strings.ReplaceAll(strings.TrimSpace(query), "-", ""))
	cutoff := time.Now().Add(-onlineTTL)
	s.mu.Lock()
	out := make([]KnownPlayer, 0, 256)
	for key, profile := range s.profiles {
		if query != "" && !strings.Contains(strings.ToLower(profile.Name), query) &&
			!strings.Contains(key, query) && !strings.Contains(profile.Server, query) {
			continue
		}
		session := s.players[key]
		out = append(out, KnownPlayer{UUID: key, Profile: *profile, Online: session != nil && session.Last.After(cutoff)})
	}
	s.mu.Unlock()
	sort.Slice(out, func(i, j int) bool { return out[i].Last.After(out[j].Last) })
	total := len(out)
	if len(out) > limit {
		out = out[:limit]
	}
	for i := range out {
		out[i].Rank = ranks.Of(out[i].UUID)
	}
	return out, total
}

// InstallRow is one launcher in the admin's install list.
type InstallRow struct {
	ID string `json:"id"`
	Install
	Online bool `json:"online"`
}

func (s *Store) Installs() []InstallRow {
	cutoff := time.Now().Add(-onlineTTL)
	s.mu.Lock()
	out := make([]InstallRow, 0, len(s.installs))
	for id, rec := range s.installs {
		out = append(out, InstallRow{ID: id, Install: *rec, Online: rec.Last.After(cutoff)})
	}
	s.mu.Unlock()
	sort.Slice(out, func(i, j int) bool { return out[i].Last.After(out[j].Last) })
	return out
}

// DaySummary is one bar in the admin's daily chart.
type DaySummary struct {
	Day       string `json:"day"`
	Players   int    `json:"players"`
	Launchers int    `json:"launchers"`
	New       int    `json:"new"`
	Peak      int    `json:"peak"`
}

// Overview is the admin dashboard's front page in one answer.
type Overview struct {
	Stats
	History    []Sample       `json:"history"`
	Days       []DaySummary   `json:"days"`
	ByMC       map[string]int `json:"byMC"`
	ByMod      map[string]int `json:"byMod"`
	ByServer   map[string]int `json:"byServer"`
	TopServers []ServerCount  `json:"topServers"`
	Playtime   int64          `json:"playtime"`
	NewPlayers int            `json:"newPlayersWeek"`
}

type ServerCount struct {
	Server  string `json:"server"`
	Online  int    `json:"online"`
	Players int    `json:"players"` // distinct players whose last server it was
}

func (s *Store) Overview() Overview {
	now := time.Now()
	cutoff := now.Add(-onlineTTL)
	week := now.Add(-7 * 24 * time.Hour)
	s.mu.Lock()
	defer s.mu.Unlock()

	ov := Overview{
		Stats:    s.statsLocked(now),
		History:  append([]Sample(nil), s.history...),
		ByMC:     map[string]int{},
		ByMod:    map[string]int{},
		ByServer: map[string]int{},
	}
	servers := map[string]*ServerCount{}
	count := func(name string) *ServerCount {
		if servers[name] == nil {
			servers[name] = &ServerCount{Server: name}
		}
		return servers[name]
	}
	for _, session := range s.players {
		if !session.Last.After(cutoff) {
			continue
		}
		ov.ByMC[orUnknown(session.MC)]++
		ov.ByMod[orUnknown(session.Mod)]++
		ov.ByServer[orUnknown(session.Server)]++
		if session.Server != "" {
			count(session.Server).Online++
		}
	}
	for _, profile := range s.profiles {
		ov.Playtime += profile.Playtime
		if profile.First.After(week) {
			ov.NewPlayers++
		}
		if profile.Server != "" {
			count(profile.Server).Players++
		}
	}
	for _, sc := range servers {
		ov.TopServers = append(ov.TopServers, *sc)
	}
	sort.Slice(ov.TopServers, func(i, j int) bool {
		a, b := ov.TopServers[i], ov.TopServers[j]
		if a.Online != b.Online {
			return a.Online > b.Online
		}
		if a.Players != b.Players {
			return a.Players > b.Players
		}
		return a.Server < b.Server
	})
	if len(ov.TopServers) > 25 {
		ov.TopServers = ov.TopServers[:25]
	}

	for i := 29; i >= 0; i-- {
		key := now.AddDate(0, 0, -i).UTC().Format("2006-01-02")
		summary := DaySummary{Day: key}
		if day := s.days[key]; day != nil {
			summary.Players, summary.Launchers, summary.New, summary.Peak = len(day.Players), len(day.Launchers), day.New, day.Peak
		}
		ov.Days = append(ov.Days, summary)
	}
	return ov
}

func orUnknown(s string) string {
	if s == "" {
		return "unknown"
	}
	return s
}

// --- housekeeping and disk --------------------------------------------------

// Maintain drops stale presence and flushes what changed. One goroutine owns
// both jobs so nothing on the request path ever waits on a disk write.
func (s *Store) Maintain() {
	go s.nameUnnamed()
	lastSample := time.Time{}
	for now := range time.Tick(30 * time.Second) {
		s.mu.Lock()
		s.sweepLocked(now)
		st := s.statsLocked(now)
		day := s.dayLocked(now)
		if st.InGame > day.Peak {
			day.Peak = st.InGame
			s.dirty = true
		}
		if now.Sub(lastSample) >= sampleEvery {
			lastSample = now
			s.history = append(s.history, Sample{T: now, InGame: st.InGame, Launchers: st.LaunchersOpen})
			if len(s.history) > maxSamples {
				s.history = s.history[len(s.history)-maxSamples:]
			}
			s.dirty = true
		}
		dirty := s.dirty
		s.dirty = false
		var installs, seen, profiles, history, days []byte
		if dirty {
			installs, _ = json.Marshal(s.installs)
			seen, _ = json.Marshal(s.seen)
			profiles, _ = json.Marshal(s.profiles)
			history, _ = json.Marshal(s.history)
			days, _ = json.Marshal(s.days)
		}
		s.mu.Unlock()
		if dirty {
			writeAtomic(filepath.Join(s.dir, "installs.json"), installs)
			writeAtomic(filepath.Join(s.dir, "players.json"), seen)
			writeAtomic(filepath.Join(s.dir, "profiles.json"), profiles)
			writeAtomic(filepath.Join(s.dir, "history.json"), history)
			writeAtomic(filepath.Join(s.dir, "days.json"), days)
		}
	}
}

// nameUnnamed asks Mojang for the names old mods never send. One a second at
// most, and only once per session, so it cannot get us rate-limited.
func (s *Store) nameUnnamed() {
	for key := range s.unnamed {
		s.mu.Lock()
		profile := s.profiles[key]
		known := profile != nil && profile.Name != ""
		s.mu.Unlock()
		if known {
			continue
		}
		name, err := resolveUUID(key)
		if err == nil && name != "" {
			s.mu.Lock()
			if profile := s.profiles[key]; profile != nil {
				profile.Name = name
			}
			if session := s.players[key]; session != nil && session.Name == "" {
				session.Name = name
			}
			s.dirty = true
			s.mu.Unlock()
		}
		time.Sleep(time.Second)
	}
}

func (s *Store) dayLocked(now time.Time) *Day {
	key := now.UTC().Format("2006-01-02")
	day := s.days[key]
	if day == nil {
		day = &Day{Players: map[string]bool{}, Launchers: map[string]bool{}}
		s.days[key] = day
		oldest := now.AddDate(0, 0, -keepDays).UTC().Format("2006-01-02")
		for k := range s.days {
			if k < oldest {
				delete(s.days, k)
			}
		}
	}
	return day
}

func (s *Store) sweepLocked(now time.Time) {
	cutoff := now.Add(-onlineTTL)
	for key, session := range s.players {
		if session.Last.Before(cutoff) {
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
	if data, err := os.ReadFile(filepath.Join(s.dir, "profiles.json")); err == nil {
		_ = json.Unmarshal(data, &s.profiles)
	}
	if data, err := os.ReadFile(filepath.Join(s.dir, "history.json")); err == nil {
		_ = json.Unmarshal(data, &s.history)
	}
	if data, err := os.ReadFile(filepath.Join(s.dir, "days.json")); err == nil {
		_ = json.Unmarshal(data, &s.days)
	}
	for _, day := range s.days {
		if day.Players == nil {
			day.Players = map[string]bool{}
		}
		if day.Launchers == nil {
			day.Launchers = map[string]bool{}
		}
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
	// Players seen before profiles existed get one from what we do know, and
	// a name from Mojang over the next while.
	for key, last := range s.seen {
		if s.profiles[key] == nil {
			s.profiles[key] = &Profile{First: last, Last: last}
		}
		if s.profiles[key].Name == "" {
			select {
			case s.unnamed <- key:
			default:
			}
		}
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

// cleanBeat keeps only what a well-behaved mod could have sent: a real
// Minecraft name, short version strings, a printable server address.
func cleanBeat(info BeatInfo) BeatInfo {
	if !validName(info.Name) {
		info.Name = ""
	}
	info.MC = printable(clip(info.MC, 24))
	info.Mod = printable(clip(info.Mod, 24))
	info.Server = strings.ToLower(printable(clip(info.Server, 96)))
	return info
}

func validName(name string) bool {
	if len(name) < 2 || len(name) > 16 {
		return false
	}
	for i := 0; i < len(name); i++ {
		c := name[i]
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c == '_') {
			return false
		}
	}
	return true
}

func printable(s string) string {
	return strings.Map(func(r rune) rune {
		if r < 0x20 || r == 0x7f {
			return -1
		}
		return r
	}, s)
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
