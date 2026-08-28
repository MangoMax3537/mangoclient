package gg.mangoclient.mangoconfig;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.packet.c2s.query.QueryPingC2SPacket;
import net.minecraft.util.Util;

/**
 * The round trip to the server, timed here rather than read off the tab list.
 *
 * The tab list only carries whatever latency the server chose to publish: it
 * starts at zero, plenty of servers never update it for the player's own
 * entry, and proxied networks report the hop behind the proxy instead. So the
 * client asks for itself, with the same play-protocol ping request vanilla's
 * own debug chart uses - the server echoes the timestamp straight back, and
 * the difference is a real measurement.
 *
 * Servers older than 1.20.2 behind a translating proxy will never answer. That
 * is what {@link #GIVE_UP_AFTER} is for: after a few silent requests this stops
 * asking, and {@link gg.mangoclient.mangoconfig.modules.PingModule} falls back
 * to the tab list for the rest of the session.
 */
public final class Ping {
	private Ping() {
	}

	/** Often enough to follow a lag spike, rarely enough to cost nothing. */
	private static final long INTERVAL_MS = 1000L;
	/** How long a single request may stay unanswered before it is written off. */
	private static final long TIMEOUT_MS = 5000L;
	/** Unanswered requests in a row before the server is taken as not supporting it. */
	private static final int GIVE_UP_AFTER = 3;

	/** Touched from the network thread as well as the client thread. */
	private static volatile int latency = -1;
	private static volatile long outstanding = -1;
	private static long lastSent;
	private static int missed;
	private static boolean supported = true;
	/** The connection the numbers above belong to, so a new server starts clean. */
	private static ClientPlayNetworkHandler connection;

	/** The last measured round trip in ms, or -1 when there is not one yet. */
	public static int latency() {
		return latency;
	}

	/** Called every client tick; sends at most one request a second. */
	public static void onClientTick(MinecraftClient mc) {
		ClientPlayNetworkHandler handler = mc.getNetworkHandler();
		if (handler == null || mc.player == null) {
			if (connection != null) reset();
			return;
		}
		if (handler != connection) {
			reset();
			connection = handler;
		}
		// The integrated server would answer, but a number measured against
		// this same process is not worth drawing.
		if (mc.isInSingleplayer() || !supported) return;

		long now = Util.getMeasuringTimeMs();
		if (outstanding >= 0 && now - lastSent >= TIMEOUT_MS) {
			outstanding = -1;
			if (++missed >= GIVE_UP_AFTER) supported = false;
		}
		if (outstanding >= 0 || now - lastSent < INTERVAL_MS) return;

		lastSent = now;
		outstanding = now;
		try {
			handler.sendPacket(new QueryPingC2SPacket(now));
		} catch (Throwable ignored) {
			// A connection that died mid-tick is not this module's problem.
			outstanding = -1;
		}
	}

	/** Called from the network handler when the server echoes a request back. */
	public static void onResult(long startTime) {
		if (startTime != outstanding) return; // a straggler from an earlier one
		outstanding = -1;
		missed = 0;
		latency = (int) Math.max(0, Math.min(Integer.MAX_VALUE, Util.getMeasuringTimeMs() - startTime));
	}

	private static void reset() {
		connection = null;
		latency = -1;
		outstanding = -1;
		lastSent = 0;
		missed = 0;
		supported = true;
	}
}
