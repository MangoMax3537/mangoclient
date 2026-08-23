package gg.mangoclient.mangoconfig;

import com.jagrosh.discordipc.IPCClient;
import com.jagrosh.discordipc.IPCListener;
import com.jagrosh.discordipc.entities.RichPresence;
import com.jagrosh.discordipc.entities.pipe.PipeStatus;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ServerInfo;
import org.json.JSONArray;
import org.json.JSONObject;

import java.time.OffsetDateTime;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;

/**
 * What Discord shows while MangoClient is running.
 *
 * Everything that can block - the pipe to Discord, the connect, the retry -
 * lives on one daemon thread. The client tick only ever writes a string into
 * {@link #desired}; the thread notices and sends. That split is the whole
 * design: a player without Discord pays a string comparison per tick, and a
 * Discord that is closed, busy or restarting can never stall a frame.
 *
 * Nothing here logs. Not running Discord is the normal case for a lot of
 * players, and a line every minute saying so is noise, not information.
 */
public final class RPCManager {
	private static final long APPLICATION_ID = 1541134992334135457L;

	/** The artwork uploaded to the Discord application, and its tooltip. */
	private static final String LARGE_IMAGE = "mangoclient";
	private static final String LARGE_IMAGE_TEXT = "Mango Client";

	/** The top line never changes; only the state below it does. */
	private static final String DETAILS = "Playing Minecraft";

	/** The button under the presence. Everyone who sees the profile can click it. */
	private static final String BUTTON_LABEL = "mangoclient.com";
	private static final String BUTTON_URL = "https://mangoclient.com";

	private static final String MENU = "In the menu";
	private static final String SINGLEPLAYER = "Singleplayer";
	private static final String OTHER_SERVER = "Playing a Server";

	/**
	 * Servers worth naming, host suffix -&gt; what Discord shows. Add a line here
	 * and that server has its own text; everything else stays "Playing a
	 * Server". A suffix rather than a substring on purpose:
	 * "mc.vincentvanilla.net" is them, "vincentvanilla.net.example.com" is not.
	 */
	private static final Map<String, String> SERVERS = new LinkedHashMap<>();

	static {
		SERVERS.put("vincentvanilla.net", "Playing VincentVanilla.net");
	}

	/** How long to wait before looking for Discord again. */
	private static final long RECONNECT_MS = 60_000;
	/** How often the sender wakes up to see whether the state moved. */
	private static final long POLL_MS = 500;
	/** Discord rate-limits presence updates; stay well under it. */
	private static final long MIN_GAP_MS = 15_000;

	/**
	 * Set once, reused for every update. Rebuilding it per update would restart
	 * the elapsed timer in Discord on every world change.
	 */
	private static OffsetDateTime start;

	/** Null whenever there is no live pipe - which is also the retry signal. */
	private static volatile IPCClient client;

	/** Written by the client tick, read by the sender thread. */
	private static volatile String desired = MENU;

	/** What Discord was last told, so an unchanged state costs nothing. */
	private static volatile String sent;
	private static volatile long lastSendMs;

	private static boolean started;
	private static volatile boolean running = true;

	private RPCManager() {
	}

	// --- lifecycle ------------------------------------------------------------

	public static void start() {
		if (started) return;
		started = true;
		start = OffsetDateTime.now();

		// Daemon, and not only so Minecraft can exit: DiscordIPC starts its own
		// reading thread inside connect(), and a new thread inherits the daemon
		// flag of the thread that created it. Connecting from here is what keeps
		// that thread from holding the JVM open after the player quits.
		Thread thread = new Thread(RPCManager::loop, "MangoConfig-DiscordRPC");
		thread.setDaemon(true);
		thread.start();

		Runtime.getRuntime().addShutdownHook(
			new Thread(RPCManager::stop, "MangoConfig-DiscordRPC-Shutdown"));
	}

	/** Tell Discord we are gone. Safe to call twice, or having never connected. */
	public static void stop() {
		running = false;
		drop();
	}

	/**
	 * Push a presence now. The tick loop goes through here too, but it is worth
	 * having on its own: anything in the client that wants to say something
	 * different for a moment can just call it.
	 */
	public static void update(String details, String state) {
		IPCClient live = client;
		if (live == null || live.getStatus() != PipeStatus.CONNECTED) return;

		live.sendRichPresence(new Linked(details, state));
		sent = key(details, state);
		lastSendMs = System.currentTimeMillis();
	}

	// --- the client tick ------------------------------------------------------

	/**
	 * Called every client tick, in a menu as much as in a world. It decides what
	 * the state should be and stops there - deciding is cheap, sending is not.
	 */
	public static void onClientTick(MinecraftClient mc) {
		if (mc == null) return;
		desired = stateFor(mc);
	}

	private static String stateFor(MinecraftClient mc) {
		if (mc.world == null) return MENU;

		// No server entry means the world is our own. A world opened to LAN
		// still has none for the host, which is right: they are the server.
		ServerInfo info = mc.getCurrentServerEntry();
		if (info == null || mc.isInSingleplayer()) return SINGLEPLAYER;

		return serverState(info.address);
	}

	/** Package-private so the matching can be exercised without a game. */
	static String serverState(String address) {
		String host = hostOf(address);
		for (Map.Entry<String, String> entry : SERVERS.entrySet()) {
			String domain = entry.getKey();
			if (host.equals(domain) || host.endsWith("." + domain)) return entry.getValue();
		}
		return OTHER_SERVER;
	}

	/**
	 * The bare host out of whatever the server list holds: a port, a bracketed
	 * IPv6 address and the trailing dot of a fully qualified name all come off.
	 */
	static String hostOf(String address) {
		if (address == null) return "";
		String host = address.trim().toLowerCase(Locale.ROOT);

		if (host.startsWith("[")) {
			int end = host.indexOf(']');
			host = end < 0 ? host.substring(1) : host.substring(1, end);
		} else {
			int colon = host.indexOf(':');
			if (colon >= 0) host = host.substring(0, colon);
		}

		while (host.endsWith(".")) host = host.substring(0, host.length() - 1);
		return host;
	}

	// --- the sender thread ----------------------------------------------------

	private static void loop() {
		while (running) {
			try {
				if (client != null) push();
				else connect();
			} catch (Throwable ignored) {
				// Discord is not running, or walked away mid-send. Either way the
				// pipe is no use now; drop it and let the next round try again.
				drop();
			}
			sleep(client == null ? RECONNECT_MS : POLL_MS);
		}
	}

	/** Throws when Discord is not there, which is the ordinary outcome. */
	private static void connect() throws Exception {
		IPCClient fresh = new IPCClient(APPLICATION_ID);
		fresh.setListener(new IPCListener() {
			@Override
			public void onClose(IPCClient ipc, JSONObject json) {
				drop();
			}

			@Override
			public void onDisconnect(IPCClient ipc, Throwable cause) {
				drop();
			}
		});
		fresh.connect();

		// A fresh pipe remembers nothing, so neither do we: the next round sends
		// the current state even though it has not changed.
		sent = null;
		lastSendMs = 0;
		client = fresh;
	}

	private static void push() {
		String state = desired;
		if (key(DETAILS, state).equals(sent)) return;
		if (System.currentTimeMillis() - lastSendMs < MIN_GAP_MS) return;
		update(DETAILS, state);
	}

	private static void drop() {
		IPCClient old = client;
		client = null;
		sent = null;
		if (old == null) return;
		try {
			old.close();
		} catch (Throwable ignored) {
			// Closing a pipe Discord already closed throws; there is nothing left
			// to do about it either way.
		}
	}

	private static String key(String details, String state) {
		return details + ' ' + state;
	}

	/**
	 * The presence, plus the button the library cannot build.
	 *
	 * DiscordIPC predates Discord's `buttons` field and its builder has no way
	 * to set one. Everything else about the payload it produces is right, so
	 * rather than hand-rolling the whole activity JSON, this lets the library
	 * build it and adds the one key it has never heard of. `secrets` goes: it
	 * is an empty object here anyway, and Discord treats an activity with
	 * join/spectate secrets as one that cannot carry buttons.
	 */
	private static final class Linked extends RichPresence {
		Linked(String details, String state) {
			super(state, details, start, null, LARGE_IMAGE, LARGE_IMAGE_TEXT, null, null,
				null, 0, 0, null, null, null, false);
		}

		@Override
		public JSONObject toJson() {
			JSONObject json = super.toJson();
			json.remove("secrets");
			return json.put("buttons", new JSONArray()
				.put(new JSONObject().put("label", BUTTON_LABEL).put("url", BUTTON_URL)));
		}
	}

	private static void sleep(long ms) {
		try {
			Thread.sleep(ms);
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
			running = false;
		}
	}
}
