package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.client.network.PlayerListEntry;

/**
 * Latency to the server.
 *
 * The player's own entry in the tab list is where the number lives, and it is
 * not there straight away: it arrives with the first player-list update after
 * joining. Until then, and in single player, the module says so instead of
 * drawing nothing - a module that shows an empty box reads as broken.
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

		PlayerListEntry entry = handler.getPlayerListEntry(mc.player.getUuid());
		return entry == null ? -1 : entry.getLatency();
	}

	private String line(MinecraftClient mc) {
		int ms = ping(mc);
		if (ms == -2) return "Local";
		if (ms < 0) return "-- ms";
		return label.value ? ms + " ms" : String.valueOf(ms);
	}

	private int colourFor(int ms) {
		if (!colourByQuality.value || ms < 0) return colour.value;
		if (ms < 80) return 0xFF1BD96A;
		if (ms < 200) return 0xFFF2A53C;
		return 0xFFFF4B4B;
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
