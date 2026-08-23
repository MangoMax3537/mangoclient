package gg.mangoclient.mangoconfig;

import gg.mangoclient.mangoconfig.compat.Compat;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.GameOptions;

import java.util.ArrayList;
import java.util.List;

/**
 * Minecraft's own settings, brought into MangoConfig.
 *
 * Nothing here keeps a value of its own: every row reads and writes the game's
 * option directly, so what is changed here is the same thing the vanilla
 * options screen changes, and it persists the way it always did. That is why
 * the rows are bound rather than saved into mangoconfig.json.
 */
public final class GameSettings {
	private GameSettings() {
	}

	private static GameOptions opts() {
		return MinecraftClient.getInstance().options;
	}

	/** Percentages are friendlier than 0.0-1.0 on a slider. */
	private static int percent(double value) {
		return (int) Math.round(value * 100);
	}

	public static List<Option> video() {
		GameOptions o = opts();
		List<Option> rows = new ArrayList<>();

		rows.add(new Option.Range("fov", "Field of view", 70, 30, 110, "")
			.bind(() -> o.getFov().getValue(), v -> o.getFov().setValue(v)));

		rows.add(new Option.Range("renderDistance", "Render distance", 12, 2, 32, " chunks")
			.bind(() -> o.getViewDistance().getValue(), v -> o.getViewDistance().setValue(v)));

		rows.add(new Option.Range("simDistance", "Simulation distance", 12, 5, 32, " chunks")
			.bind(() -> o.getSimulationDistance().getValue(), v -> o.getSimulationDistance().setValue(v)));

		rows.add(new Option.Range("maxFps", "Max framerate", 120, 10, 260, " fps")
			.bind(() -> o.getMaxFps().getValue(), v -> o.getMaxFps().setValue(v)));

		rows.add(new Option.Range("gamma", "Brightness", 50, 0, 100, "%")
			.bind(() -> percent(o.getGamma().getValue()), v -> o.getGamma().setValue(v / 100.0)));

		rows.add(new Option.Range("guiScale", "GUI scale", 0, 0, 4, "")
			.bind(() -> o.getGuiScale().getValue(), v -> o.getGuiScale().setValue(v))
			.hint("0 means automatic."));

		rows.add(new Option.Bool("vsync", "VSync", true)
			.bind(() -> o.getEnableVsync().getValue(), v -> o.getEnableVsync().setValue(v)));

		// Fullscreen is deliberately absent. Setting the option alone does not do
		// what vanilla's own screen does - vanilla toggles the window there and
		// then - so it only takes hold on the next launch, and a borderless
		// fullscreen mod that hooks the window's monitor lookup can find itself
		// running before its own config is ready. That crashed a real instance.
		// Vanilla's video settings still have the toggle; this is not the place.

		rows.add(new Option.Bool("bobView", "View bobbing", true)
			.bind(() -> o.getBobView().getValue(), v -> o.getBobView().setValue(v)));

		rows.add(new Option.Bool("entityShadows", "Entity shadows", true)
			.bind(() -> o.getEntityShadows().getValue(), v -> o.getEntityShadows().setValue(v)));

		// The vignette toggle only exists from 1.21.11 on; older versions skip the row.
		var vignette = Compat.vignette(o);
		if (vignette != null) {
			rows.add(new Option.Bool("vignette", "Vignette", true)
				.bind(() -> vignette.getValue(), v -> vignette.setValue(v)));
		}

		rows.add(new Option.Range("distortion", "Distortion effects", 100, 0, 100, "%")
			.bind(() -> percent(o.getDistortionEffectScale().getValue()),
				v -> o.getDistortionEffectScale().setValue(v / 100.0))
			.hint("0% takes the wobble out of a nether portal."));

		rows.add(new Option.Range("fovEffects", "Field of view effects", 100, 0, 100, "%")
			.bind(() -> percent(o.getFovEffectScale().getValue()),
				v -> o.getFovEffectScale().setValue(v / 100.0))
			.hint("0% keeps the view steady while sprinting."));

		return rows;
	}

	public static List<Option> controls() {
		GameOptions o = opts();
		List<Option> rows = new ArrayList<>();

		rows.add(new Option.Range("sensitivity", "Mouse sensitivity", 50, 0, 200, "%")
			.bind(() -> percent(o.getMouseSensitivity().getValue() * 2),
				v -> o.getMouseSensitivity().setValue(v / 200.0)));

		rows.add(new Option.Bool("sprintToggle", "Toggle sprint", false)
			.bind(() -> o.getSprintToggled().getValue(), v -> o.getSprintToggled().setValue(v)));

		rows.add(new Option.Bool("sneakToggle", "Toggle sneak", false)
			.bind(() -> o.getSneakToggled().getValue(), v -> o.getSneakToggled().setValue(v)));

		rows.add(new Option.Bool("autoJump", "Auto jump", false)
			.bind(() -> o.getAutoJump().getValue(), v -> o.getAutoJump().setValue(v)));

		rows.add(new Option.Bool("subtitles", "Subtitles", false)
			.bind(() -> o.getShowSubtitles().getValue(), v -> o.getShowSubtitles().setValue(v)));

		rows.add(new Option.Range("chatOpacity", "Chat opacity", 100, 0, 100, "%")
			.bind(() -> percent(o.getChatOpacity().getValue()), v -> o.getChatOpacity().setValue(v / 100.0)));

		rows.add(new Option.Range("chatScale", "Chat scale", 100, 0, 100, "%")
			.bind(() -> percent(o.getChatScale().getValue()), v -> o.getChatScale().setValue(v / 100.0)));

		return rows;
	}

	/** Write Minecraft's options back to disk; call when the screen closes. */
	public static void persist() {
		try {
			opts().write();
		} catch (Exception e) {
			// Not worth interrupting a session over.
		}
	}

	/**
	 * Full brightness without a mixin: gamma is a normal option, and pinning it
	 * to its maximum is what every "fullbright" toggle ultimately does.
	 */
	public static Option.Bool fullbright() {
		GameOptions o = opts();
		return (Option.Bool) new Option.Bool("fullbright", "Full brightness", false)
			.bind(() -> o.getGamma().getValue() >= 0.99,
				v -> o.getGamma().setValue(v ? 1.0 : 0.5))
			.hint("Pins brightness to its maximum.");
	}
}
