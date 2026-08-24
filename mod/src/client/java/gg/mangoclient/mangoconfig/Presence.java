package gg.mangoclient.mangoconfig;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import gg.mangoclient.mangoconfig.compat.Compat;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.text.MutableText;
import net.minecraft.text.Style;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;
import net.minecraft.util.Identifier;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;

/**
 * Who else is on MangoClient right now, and what they are to it.
 *
 * A background thread tells the presence server "I am online" and asks which
 * of the players around us are too, and which of those hold a rank. The render
 * hooks only ever touch {@link #isMango(UUID)} and {@link #withBadge(Text, UUID)}:
 * two volatile fields and some text building, nothing that can block a frame on
 * the network.
 */
public final class Presence {
	private static final String LEGACY_BASE = "http://94.249.184.45:8880";
	private static final String BASE = apiBase();
	private static final Duration TIMEOUT = Duration.ofSeconds(5);

	/** How often we tell the server we are here. */
	private static final long BEAT_MS = 30_000;
	/** How often we re-ask about a tab list that is not changing. */
	private static final long REFRESH_MS = 30_000;
	/** The fastest we will re-ask after it does change. */
	private static final long MIN_GAP_MS = 3_000;
	/** How often we glance at the tab list. Cheap: it is a list copy. */
	private static final long POLL_MS = 500;

	/**
	 * A glyph per rank rather than one glyph tinted four ways: the mango is the
	 * app's own logo, and a tint could only have multiplied it - the green leaf
	 * would have survived into the gold and the blue. See mango.json, where the
	 * four bitmaps sit at U+E000 upwards in the order used here.
	 */
	private static final String GLYPH_MEMBER = "";
	private static final Map<String, String> RANK_GLYPHS = Map.of(
		"mangoplus", "", // the logo in its own colours
		"owner", "",     // gold
		"support", ""    // blue
	);

	/**
	 * White, so each bitmap is drawn exactly as it was painted; italic off so
	 * /nick styles and team formatting cannot lean the mango over, bold off so
	 * they cannot fatten it either.
	 */
	private static final Style BADGE_STYLE = Compat
		.withFont(Style.EMPTY, Identifier.of("mangoconfig", "mango"))
		.withColor(Formatting.WHITE)
		.withItalic(false)
		.withBold(false);

	/** UUIDs the server vouched for; swapped whole, never mutated. */
	private static volatile Set<UUID> online = Set.of();
	/** Ranks for the players we last asked about, same rule. */
	private static volatile Map<UUID, String> ranks = Map.of();
	/** Our own rank, which the heartbeat answers with, so it needs no query. */
	private static volatile String selfRank = "";

	private Presence() {
	}

	private static String apiBase() {
		String raw = System.getProperty("mangoconfig.apiBase", LEGACY_BASE).replaceAll("/+$", "");
		try {
			URI uri = URI.create(raw);
			String path = uri.getRawPath();
			boolean originOnly = uri.getHost() != null && uri.getUserInfo() == null
				&& uri.getRawQuery() == null && uri.getRawFragment() == null
				&& (path == null || path.isEmpty());
			if (originOnly && ("https".equalsIgnoreCase(uri.getScheme()) || LEGACY_BASE.equals(raw))) return raw;
		} catch (IllegalArgumentException ignored) {
		}
		System.err.println("[MangoConfig] mangoconfig.apiBase must be HTTPS; using the legacy presence endpoint");
		return LEGACY_BASE;
	}

	public static void init() {
		Thread t = new Thread(Presence::loop, "MangoConfig-Presence");
		t.setDaemon(true);
		t.start();
	}

	public static boolean badgeEnabled() {
		return Mods.mangoBadge.value;
	}

	/** Yourself always counts: your own badge must not depend on the network. */
	public static boolean isMango(UUID uuid) {
		if (uuid == null) return false;
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc != null && mc.player != null && uuid.equals(mc.player.getUuid())) return true;
		return online.contains(uuid);
	}

	/** This player's mango, a space, then the name as it was. */
	public static Text withBadge(Text name, UUID uuid) {
		MutableText out = Text.empty();
		out.append(Text.literal(glyphFor(uuid)).setStyle(BADGE_STYLE));
		out.append(" ");
		out.append(name);
		return out;
	}

	private static String glyphFor(UUID uuid) {
		String rank = null;
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc != null && mc.player != null && mc.player.getUuid().equals(uuid)) {
			rank = selfRank;
		}
		if (rank == null || rank.isEmpty()) rank = ranks.get(uuid);
		if (rank == null) return GLYPH_MEMBER;
		return RANK_GLYPHS.getOrDefault(rank, GLYPH_MEMBER);
	}

	// --- the background loop --------------------------------------------------

	/**
	 * Polls twice a second but only speaks when it has something to say: the
	 * first tick after joining, whenever the tab list changes (at most every
	 * {@link #MIN_GAP_MS}), and every {@link #REFRESH_MS} otherwise. That is
	 * what puts the badges on screen as the world loads instead of half a
	 * minute into it.
	 */
	private static void loop() {
		HttpClient http = HttpClient.newBuilder().connectTimeout(TIMEOUT).build();
		Set<UUID> asked = Set.of();
		long lastBeat = 0;
		long lastQuery = 0;

		while (true) {
			try {
				Thread.sleep(POLL_MS);

				MinecraftClient mc = MinecraftClient.getInstance();
				if (mc == null || mc.player == null || mc.getNetworkHandler() == null) {
					// Out of a world: forget everyone, and let the next join
					// start over with an immediate heartbeat and query.
					forget();
					asked = Set.of();
					lastBeat = 0;
					lastQuery = 0;
					continue;
				}

				long now = System.currentTimeMillis();
				if (now - lastBeat >= BEAT_MS) {
					// Not gated on the toggle: switching the badge off hides the
					// mangos from you, it does not hide you from everyone else.
					lastBeat = now;
					beat(http, mc.player.getUuid());
				}

				if (!badgeEnabled()) {
					forget();
					asked = Set.of();
					continue;
				}

				Set<UUID> tab = tabList(mc);
				if (tab.isEmpty()) {
					forget();
					asked = Set.of();
					continue;
				}

				boolean changed = !tab.equals(asked);
				boolean due = now - lastQuery >= REFRESH_MS;
				if (!due && !(changed && now - lastQuery >= MIN_GAP_MS)) continue;

				// Stamped before the request, not after: a server that is down
				// must not turn this into a request every half second.
				asked = tab;
				lastQuery = now;
				query(http, tab);
			} catch (InterruptedException e) {
				return;
			} catch (Exception e) {
				// Server down or no internet: keep the last answer, try later.
			}
		}
	}

	private static void forget() {
		online = Set.of();
		ranks = Map.of();
	}

	/** The tab list belongs to the client thread; copy it there, use it here. */
	private static Set<UUID> tabList(MinecraftClient mc) throws Exception {
		return CompletableFuture.supplyAsync(() -> {
			Set<UUID> ids = new LinkedHashSet<>();
			if (mc.getNetworkHandler() != null) {
				for (PlayerListEntry entry : mc.getNetworkHandler().getPlayerList()) {
					UUID id = Compat.profileId(entry.getProfile());
					if (id != null) ids.add(id);
				}
			}
			return ids;
		}, mc).get();
	}

	private static void beat(HttpClient http, UUID self) throws Exception {
		JsonObject json = new JsonObject();
		json.addProperty("uuid", self.toString());
		String response = post(http, "/v1/heartbeat", json.toString());
		JsonElement rank = JsonParser.parseString(response).getAsJsonObject().get("rank");
		selfRank = rank == null || rank.isJsonNull() ? "" : rank.getAsString();
	}

	private static void query(HttpClient http, Set<UUID> uuids) throws Exception {
		JsonArray array = new JsonArray();
		for (UUID uuid : uuids) array.add(uuid.toString());
		JsonObject body = new JsonObject();
		body.add("uuids", array);

		JsonObject answer = JsonParser.parseString(post(http, "/v1/query", body.toString())).getAsJsonObject();

		Set<UUID> fresh = new LinkedHashSet<>();
		JsonArray onlineArray = answer.getAsJsonArray("online");
		if (onlineArray != null) {
			for (JsonElement element : onlineArray) {
				UUID uuid = parse(element.getAsString());
				if (uuid != null) fresh.add(uuid);
			}
		}

		// Ranks arrived with the 1.9.0 server; an older one simply leaves them
		// out and every mango stays grey.
		Map<UUID, String> freshRanks = new HashMap<>();
		JsonElement rankElement = answer.get("ranks");
		if (rankElement != null && rankElement.isJsonObject()) {
			for (Map.Entry<String, JsonElement> entry : rankElement.getAsJsonObject().entrySet()) {
				UUID uuid = parse(entry.getKey());
				if (uuid != null && entry.getValue().isJsonPrimitive()) {
					freshRanks.put(uuid, entry.getValue().getAsString());
				}
			}
		}

		online = Set.copyOf(fresh);
		ranks = Map.copyOf(freshRanks);
	}

	private static UUID parse(String raw) {
		try {
			return UUID.fromString(raw);
		} catch (IllegalArgumentException e) {
			return null;
		}
	}

	private static String post(HttpClient http, String path, String body) throws Exception {
		HttpRequest request = HttpRequest.newBuilder(URI.create(BASE + path))
			.timeout(TIMEOUT)
			.header("Content-Type", "application/json")
			.POST(HttpRequest.BodyPublishers.ofString(body))
			.build();
		HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
		if (response.statusCode() / 100 != 2) throw new IllegalStateException("HTTP " + response.statusCode());
		return response.body();
	}
}
