package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;

import java.util.List;

/** How long this session has been going. */
public class SessionModule extends TextModule {
	private static final long LAUNCHED_AT = System.currentTimeMillis();

	private final Option.Choice from = new Option.Choice("from", "Counting from", 0, "World join", "Launch");
	private final Option.Bool label = new Option.Bool("label", "Show the label", false);

	private ClientWorld world;
	private long joinedAt = LAUNCHED_AT;

	public SessionModule() {
		super("session", "Session", "How long you have been playing.", false, 0.01f, 0.30f);
		options.add(from);
		options.add(label);
	}

	/** Restart the count whenever the player lands in a different world. */
	private void follow(MinecraftClient mc) {
		if (mc.world != world) {
			world = mc.world;
			joinedAt = System.currentTimeMillis();
		}
	}

	private static String clock(long millis) {
		long seconds = Math.max(0, millis / 1000);
		long hours = seconds / 3600;
		long minutes = (seconds % 3600) / 60;
		if (hours > 0) return String.format("%d:%02d:%02d", hours, minutes, seconds % 60);
		return String.format("%d:%02d", minutes, seconds % 60);
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		follow(mc);
		long since = from.is("Launch") ? LAUNCHED_AT : joinedAt;
		String value = clock(System.currentTimeMillis() - since);
		return List.of(label.value ? "Session " + value : value);
	}
}
