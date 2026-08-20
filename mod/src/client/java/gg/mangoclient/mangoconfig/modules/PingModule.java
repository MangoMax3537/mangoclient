package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.client.network.PlayerListEntry;

public class PingModule extends HudModule {
	private final Option.Bool colourByQuality = new Option.Bool("quality", "Nach Qualität färben", true);

	public PingModule() {
		super("ping", "Ping", "Latenz zum Server.", false, 0.01f, 0.14f);
		options.add(colourByQuality);
	}

	/** -1 when there is no server to measure against, which single player is. */
	private int ping(MinecraftClient mc) {
		ClientPlayNetworkHandler handler = mc.getNetworkHandler();
		if (handler == null || mc.player == null) return -1;
		PlayerListEntry entry = handler.getPlayerListEntry(mc.player.getUuid());
		return entry == null ? -1 : entry.getLatency();
	}

	private String line(MinecraftClient mc, boolean preview) {
		int ms = ping(mc);
		if (ms < 0) return preview ? "-- ms" : "";
		return ms + " ms";
	}

	private int colourFor(int ms) {
		if (!colourByQuality.value || ms < 0) return colour.value;
		if (ms < 80) return 0xFF1BD96A;
		if (ms < 200) return 0xFFF2A53C;
		return 0xFFFF4B4B;
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		return Math.max(1, font.getWidth(line(mc, true)));
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return font.fontHeight;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		String line = line(mc, preview);
		if (line.isEmpty()) return;
		ctx.drawText(font, line, 0, 0, colourFor(ping(mc)), shadow.value);
	}
}
