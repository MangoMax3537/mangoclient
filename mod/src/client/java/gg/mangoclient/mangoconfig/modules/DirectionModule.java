package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.Direction;

import java.util.ArrayList;
import java.util.List;

/** Which way the player is facing, the way a compass mod says it. */
public class DirectionModule extends TextModule {
	private final Option.Bool axis = new Option.Bool("axis", "Show the axis", true);
	private final Option.Bool degrees = new Option.Bool("degrees", "Show degrees", false);

	public DirectionModule() {
		super("direction", "Direction", "The compass direction you are facing.", false, 0.5f, 0.02f);
		options.add(axis);
		options.add(degrees);
		axis.hint = "\"South (+Z)\" instead of just \"South\".";
	}

	private static String label(Direction facing) {
		return switch (facing) {
			case NORTH -> "North (-Z)";
			case SOUTH -> "South (+Z)";
			case WEST -> "West (-X)";
			case EAST -> "East (+X)";
			default -> "";
		};
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> out = new ArrayList<>();
		if (mc.player == null) {
			if (preview) out.add(axis.value ? "South (+Z)" : "South");
			return out;
		}
		String full = label(mc.player.getHorizontalFacing());
		String line = axis.value ? full : full.substring(0, full.indexOf(' '));
		if (degrees.value) {
			float yaw = ((mc.player.getYaw() % 360f) + 360f) % 360f;
			line += " " + Math.round(yaw) + "°";
		}
		out.add(line);
		return out;
	}
}
