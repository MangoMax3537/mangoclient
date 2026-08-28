package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.Ping;
import gg.mangoclient.mangoconfig.Theme;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.client.network.PlayerListEntry;
import net.minecraft.client.network.ServerInfo;

/**
 * Latency to the server.
 *
 * The number comes from {@link Ping}, which times a round trip of its own -
 * the tab list is only asked when a server never answers one, because what it
 * publishes for the player's own entry starts at zero, is often left there,
 * and on a proxied network describes the wrong hop entirely.
 */
public class PingModule extends HudModule {
	private final Option.Bool colourByQuality = new Option.Bool("quality", "Colour by quality", true);
	private final Option.Bool label = new Option.Bool("label", "Show \"ms\"", true);

	public PingModule() {
		super("ping", "Ping", "Latency to the server.", false, 0.01f, 0.14f);
		options.add(colourByQuality);
		options.add(label);
	}

	/** -1 when there is nothing to measure yet, -2 in single player. */
	private int ping(MinecraftClient mc) {
		if (mc.player == null) return -1;
		ClientPlayNetworkHandler handler = mc.getNetworkHandler();
		if (handler == null) return -1;
		if (mc.isInSingleplayer()) return -2;

		int measured = Ping.latency();
		if (measured >= 0) return measured;

		PlayerListEntry entry = handler.getPlayerListEntry(mc.player.getUuid());
		int tabLatency = entry == null ? -1 : entry.getLatency();
		if (tabLatency > 0) return tabLatency;

		// Nothing has come back yet. The server browser measured this same
		// connection before the join, so it stands in for the first second.
		ServerInfo server = mc.getCurrentServerEntry();
		if (server != null && server.ping > 0) {
			return (int) Math.min(server.ping, 9999L);
		}
		return -1;
	}

	private String line(MinecraftClient mc) {
		int ms = ping(mc);
		if (ms == -2) return "Local";
		if (ms < 0) return "-- ms";
		return label.value ? ms + " ms" : String.valueOf(ms);
	}

	private int colourFor(int ms) {
		if (!colourByQuality.value || ms < 0) return colour.value;
		if (ms < 80) return Theme.OK;
		if (ms < 200) return Theme.WARN;
		return Theme.DANGER;
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		// Sized for the widest it gets, so the plate does not jump about.
		return Math.max(font.getWidth(line(mc)), font.getWidth("999 ms"));
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return font.fontHeight;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		ctx.drawText(font, line(mc), 0, 0, colourFor(ping(mc)), shadow.value);
	}
}
