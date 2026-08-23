package gg.mangoclient.mangoconfig;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.text.MutableText;
import net.minecraft.text.Style;
import net.minecraft.text.StyleSpriteSource;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;
import net.minecraft.util.Identifier;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;

/**
 * Who else is on MangoClient right now.
 *
 * A background thread tells the presence server "I am online" and asks which
 * of the players around us are too. The render hooks only ever touch
 * {@link #isMango(UUID)} and {@link #withBadge(Text)}: a volatile set and some
 * text building, nothing that can block a frame on the network.
 */
public final class Presence {
	private static final String BASE = "http://94.249.184.45:8880";
	/** The mango glyph in the badge font; see assets/mangoconfig/font/mango.json. */
	private static final String GLYPH = "";
	private static final Duration TIMEOUT = Duration.ofSeconds(5);
	private static final long PERIOD_MS = 30_000;

	/**
	 * White, because bitmap glyphs are tinted by the text colour and the
	 * texture already carries its grey; italic off so /nick styles and team
	 * formatting cannot lean the mango over.
	 */
	private static final Style BADGE_STYLE = Style.EMPTY
		.withFont(new StyleSpriteSource.Font(Identifier.of("mangoconfig", "mango")))
		.withColor(Formatting.WHITE)
		.withItalic(false)
		.withBold(false);

	/** UUIDs the server vouched for; swapped whole, never mutated. */
	private static volatile Set<UUID> online = Set.of();

	private Presence() {
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

	/** The grey mango, a space, then the name as it was. */
	public static Text withBadge(Text name) {
		MutableText out = Text.empty();
		out.append(Text.literal(GLYPH).setStyle(BADGE_STYLE));
		out.append(" ");
		out.append(name);
		return out;
	}

	// --- the background loop --------------------------------------------------

	private static void loop() {
		HttpClient http = HttpClient.newBuilder().connectTimeout(TIMEOUT).build();
		while (true) {
			try {
				Thread.sleep(PERIOD_MS);
				tick(http);
			} catch (InterruptedException e) {
				return;
			} catch (Exception e) {
				// Server down or no internet: keep the last answer and try again.
			}
		}
	}

	private static void tick(HttpClient http) throws Exception {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc == null || mc.player == null || mc.getNetworkHandler() == null) return;

		// Not gated on the toggle: switching the badge off hides the mangos
		// from you, it does not hide you from everyone else.
		UUID self = mc.player.getUuid();
		post(http, "/v1/heartbeat", heartbeatBody(self));

		if (!badgeEnabled()) {
			online = Set.of();
			return;
		}

		// The tab list belongs to the client thread; copy it there, use it here.
		List<UUID> uuids = CompletableFuture.supplyAsync(() -> {
			List<UUID> list = new ArrayList<>();
			if (mc.getNetworkHandler() != null) {
				for (PlayerListEntry entry : mc.getNetworkHandler().getPlayerList()) {
					list.add(entry.getProfile().id());
				}
			}
			return list;
		}, mc).get();

		if (uuids.isEmpty()) {
			online = Set.of();
			return;
		}

		String response = post(http, "/v1/query", queryBody(uuids));
		Set<UUID> fresh = new HashSet<>();
		for (var element : JsonParser.parseString(response).getAsJsonObject().getAsJsonArray("online")) {
			try {
				fresh.add(UUID.fromString(element.getAsString()));
			} catch (IllegalArgumentException ignored) {
			}
		}
		online = Set.copyOf(fresh);
	}

	private static String heartbeatBody(UUID self) {
		JsonObject json = new JsonObject();
		json.addProperty("uuid", self.toString());
		return json.toString();
	}

	private static String queryBody(List<UUID> uuids) {
		JsonArray array = new JsonArray();
		for (UUID uuid : uuids) array.add(uuid.toString());
		JsonObject json = new JsonObject();
		json.add("uuids", array);
		return json.toString();
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
