package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.ArrayList;
import java.util.List;

public class CoordinatesModule extends HudModule {
	private static final String[] COMPASS = {"South", "South-west", "West", "North-west", "North", "North-east", "East", "South-east"};

	private final Option.Bool decimals = new Option.Bool("decimals", "Decimals", false);
	private final Option.Bool facing = new Option.Bool("facing", "Facing", true);
	private final Option.Choice layout = new Option.Choice("layout", "Layout", 0, "Stacked", "One line");

	public CoordinatesModule() {
		super("coords", "Coordinates", "Position and facing.", true, 0.01f, 0.06f);
		options.add(decimals);
		options.add(facing);
		options.add(layout);
	}

	private String num(double v) {
		return decimals.value ? String.format("%.1f", v) : String.valueOf((int) Math.floor(v));
	}

	private List<String> lines(MinecraftClient mc) {
		List<String> out = new ArrayList<>();
		if (mc.player == null) {
			out.add("X 0  Y 0  Z 0");
			return out;
		}
		String x = "X " + num(mc.player.getX());
		String y = "Y " + num(mc.player.getY());
		String z = "Z " + num(mc.player.getZ());
		if (layout.is("One line")) {
			out.add(x + "  " + y + "  " + z);
		} else {
			out.add(x);
			out.add(y);
			out.add(z);
		}
		if (facing.value) out.add(direction(mc));
		return out;
	}

	private String direction(MinecraftClient mc) {
		float yaw = mc.player.getYaw() % 360;
		if (yaw < 0) yaw += 360;
		return COMPASS[Math.round(yaw / 45f) % 8];
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		int w = 0;
		for (String line : lines(mc)) w = Math.max(w, font.getWidth(line));
		return w;
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return lines(mc).size() * (font.fontHeight + 1);
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		List<String> lines = lines(mc);
		for (int i = 0; i < lines.size(); i++) {
			text(ctx, font, lines.get(i), 0, i * (font.fontHeight + 1));
		}
	}
}
