package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;

/**
 * The time and the day inside the world.
 *
 * Minecraft counts a day as 24000 ticks starting at dawn, which is six in the
 * morning; everything here follows from that.
 */
public class DayModule extends TextModule {
	private static final long DAY_TICKS = 24000;

	private final Option.Bool day = new Option.Bool("day", "Show the day number", true);
	private final Option.Bool clock = new Option.Bool("clock", "Show the time", true);
	private final Option.Bool twelveHour = new Option.Bool("twelve", "12-hour clock", false);
	private final Option.Bool phase = new Option.Bool("phase", "Show day or night", false);

	public DayModule() {
		super("worldtime", "World time", "The day and time in game.", false, 0.86f, 0.06f);
		options.add(day);
		options.add(clock);
		options.add(twelveHour);
		options.add(phase);
	}

	private long timeOfDay(MinecraftClient mc) {
		return mc.world == null ? 0 : mc.world.getTimeOfDay();
	}

	private String time(long ticks) {
		long inDay = ((ticks % DAY_TICKS) + DAY_TICKS) % DAY_TICKS;
		int hour = (int) ((inDay / 1000 + 6) % 24);
		int minute = (int) ((inDay % 1000) * 60 / 1000);
		if (!twelveHour.value) return String.format("%02d:%02d", hour, minute);

		String suffix = hour < 12 ? " am" : " pm";
		int shown = hour % 12;
		if (shown == 0) shown = 12;
		return String.format("%d:%02d%s", shown, minute, suffix);
	}

	/** Night is when the sky goes dark: 13000 to 23000 ticks. */
	private boolean isNight(long ticks) {
		long inDay = ((ticks % DAY_TICKS) + DAY_TICKS) % DAY_TICKS;
		return inDay >= 13000 && inDay < 23000;
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		long ticks = timeOfDay(mc);
		List<String> out = new ArrayList<>();
		if (day.value) out.add("Day " + (ticks / DAY_TICKS + 1));
		if (clock.value) out.add(time(ticks));
		if (phase.value) out.add(isNight(ticks) ? "Night" : "Day");
		if (out.isEmpty() && preview) out.add("Day 1");
		return out;
	}
}
