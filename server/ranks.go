package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
)

/*
The staff list.

Ranks are kept by uuid, not by name: a support member who changes their IGN
keeps their blue mango, and nobody can inherit a rank by claiming an abandoned
name. Names are still stored, refreshed in the background, because the website
and the admin page have to show something a human recognises.
*/

// Ordered strongest first: a player in two lists gets the first one.
var rankOrder = []string{"owner", "support", "mangoplus"}

var rankLabels = map[string]string{
	"owner":     "Owner",
	"support":   "Support",
	"mangoplus": "Mango+",
}

type Member struct {
	Name  string    `json:"name"`
	UUID  string    `json:"uuid"` // normalised: lowercase, no dashes
	Added time.Time `json:"added"`
}

type Ranks struct {
	mu     sync.RWMutex
	byRank map[string][]Member
	byUUID map[string]string // uuid -> rank, rebuilt on every change
	path   string
}

func NewRanks(dir string) *Ranks {
	r := &Ranks{byRank: map[string][]Member{}, byUUID: map[string]string{}, path: filepath.Join(dir, "ranks.json")}
	r.load()
	return r
}

func validRank(rank string) bool {
	_, ok := rankLabels[rank]
	return ok
}

// Of answers what the mod asks for every player on the tab list.
func (r *Ranks) Of(uuid string) string {
	key := normaliseUUID(uuid)
	if key == "" {
		return ""
	}
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.byUUID[key]
}

// Lookup builds the uuid -> rank map for one query, skipping the ranked-less.
func (r *Ranks) Lookup(uuids []string) map[string]string {
	out := map[string]string{}
	r.mu.RLock()
	defer r.mu.RUnlock()
	for _, raw := range uuids {
		if rank, ok := r.byUUID[normaliseUUID(raw)]; ok {
			out[raw] = rank
		}
	}
	return out
}

// All is the website's view: every list, strongest rank first, oldest member
// first inside a list so the founding staff stay at the top.
func (r *Ranks) All() map[string][]Member {
	r.mu.RLock()
	defer r.mu.RUnlock()
	out := map[string][]Member{}
	for _, rank := range rankOrder {
		list := append([]Member(nil), r.byRank[rank]...)
		sort.SliceStable(list, func(i, j int) bool { return list[i].Added.Before(list[j].Added) })
		out[rank] = list
	}
	return out
}

// Add puts a name in a list, resolving it against Mojang first so a typo fails
// here rather than silently never matching anyone in-game.
func (r *Ranks) Add(rank, name string) (Member, error) {
	if !validRank(rank) {
		return Member{}, errors.New("unknown rank")
	}
	profile, err := resolveName(name)
	if err != nil {
		return Member{}, err
	}

	r.mu.Lock()
	defer r.mu.Unlock()
	// One rank per player: adding to a second list moves them.
	for existing := range r.byRank {
		r.removeLocked(existing, profile.UUID)
	}
	member := Member{Name: profile.Name, UUID: profile.UUID, Added: time.Now()}
	r.byRank[rank] = append(r.byRank[rank], member)
	r.reindexLocked()
	r.saveLocked()
	return member, nil
}

func (r *Ranks) Remove(rank, uuid string) error {
	if !validRank(rank) {
		return errors.New("unknown rank")
	}
	key := normaliseUUID(uuid)
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.removeLocked(rank, key) {
		return errors.New("not in that list")
	}
	r.reindexLocked()
	r.saveLocked()
	return nil
}

func (r *Ranks) removeLocked(rank, uuid string) bool {
	list := r.byRank[rank]
	for i, m := range list {
		if m.UUID == uuid {
			r.byRank[rank] = append(list[:i], list[i+1:]...)
			return true
		}
	}
	return false
}

func (r *Ranks) reindexLocked() {
	index := map[string]string{}
	// Walked weakest-first so a stronger rank overwrites it.
	for i := len(rankOrder) - 1; i >= 0; i-- {
		for _, m := range r.byRank[rankOrder[i]] {
			index[m.UUID] = rankOrder[i]
		}
	}
	r.byUUID = index
}

func (r *Ranks) load() {
	data, err := os.ReadFile(r.path)
	if err != nil {
		return
	}
	var stored map[string][]Member
	if json.Unmarshal(data, &stored) != nil {
		return
	}
	for rank, list := range stored {
		if !validRank(rank) {
			continue
		}
		for i := range list {
			list[i].UUID = normaliseUUID(list[i].UUID)
		}
		r.byRank[rank] = list
	}
	r.reindexLocked()
}

func (r *Ranks) saveLocked() {
	data, err := json.MarshalIndent(r.byRank, "", "  ")
	if err != nil {
		return
	}
	writeAtomic(r.path, data)
}

// RefreshNames keeps the displayed IGNs honest after a name change. Slow on
// purpose: Mojang rate-limits, and a stale name for an hour costs nothing.
func (r *Ranks) RefreshNames() {
	for {
		time.Sleep(6 * time.Hour)
		r.mu.RLock()
		var all []Member
		for _, list := range r.byRank {
			all = append(all, list...)
		}
		r.mu.RUnlock()

		for _, m := range all {
			name, err := resolveUUID(m.UUID)
			if err != nil || name == "" || name == m.Name {
				time.Sleep(time.Second)
				continue
			}
			r.mu.Lock()
			for rank, list := range r.byRank {
				for i := range list {
					if list[i].UUID == m.UUID {
						r.byRank[rank][i].Name = name
					}
				}
			}
			r.saveLocked()
			r.mu.Unlock()
			time.Sleep(time.Second)
		}
	}
}

// --- Mojang -----------------------------------------------------------------

type profile struct {
	Name string
	UUID string
}

var mojang = &http.Client{Timeout: 8 * time.Second}

func resolveName(name string) (profile, error) {
	name = strings.TrimSpace(name)
	if name == "" || len(name) > 16 {
		return profile{}, errors.New("that is not a Minecraft name")
	}
	body, status, err := mojangGet("https://api.mojang.com/users/profiles/minecraft/" + name)
	if err != nil {
		return profile{}, fmt.Errorf("Mojang is not answering: %w", err)
	}
	if status == http.StatusNotFound || status == http.StatusNoContent {
		return profile{}, fmt.Errorf("no Minecraft account called %q", name)
	}
	if status != http.StatusOK {
		return profile{}, fmt.Errorf("Mojang said %d", status)
	}
	var parsed struct {
		ID   string `json:"id"`
		Name string `json:"name"`
	}
	if err := json.Unmarshal(body, &parsed); err != nil || normaliseUUID(parsed.ID) == "" {
		return profile{}, errors.New("Mojang sent something unreadable")
	}
	return profile{Name: parsed.Name, UUID: normaliseUUID(parsed.ID)}, nil
}

func resolveUUID(uuid string) (string, error) {
	body, status, err := mojangGet("https://sessionserver.mojang.com/session/minecraft/profile/" + uuid)
	if err != nil || status != http.StatusOK {
		return "", errors.New("lookup failed")
	}
	var parsed struct {
		Name string `json:"name"`
	}
	_ = json.Unmarshal(body, &parsed)
	return parsed.Name, nil
}

func mojangGet(url string) ([]byte, int, error) {
	resp, err := mojang.Get(url)
	if err != nil {
		return nil, 0, err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	return body, resp.StatusCode, err
}
