package gg.mangoclient.mangoconfig;

import gg.mangoclient.mangoconfig.compat.Compat;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.HandledScreen;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.client.util.Window;
import net.minecraft.entity.effect.StatusEffects;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;

/**
 * The mods that change how the world and vanilla's own screens look: the block
 * outline, light, colour, fog, hitboxes, chat, F3, tab list and scoreboard,
 * and how big an inventory is drawn.
 *
 * Same shape as Mods and Pvp: plain settings for the screen, and a small piece
 * of logic per mod that the mixins call into. Every mixin hands vanilla's own
 * value through untouched while its mod is off.
 */
public final class Visuals {
	private Visuals() {
	}

	// --- block overlay -------------------------------------------------------

	public static final Option.Bool blockOverlay = new Option.Bool("blockOverlay", "Block overlay", false);
	public static final Option.Colour outlineColour = new Option.Colour("outlineColour", "Outline colour", Theme.BRAND);
	public static final Option.Range outlineOpacity =
		new Option.Range("outlineOpacity", "Outline opacity", 80, 10, 100, "%");

	/** The outline as ARGB; vanilla's colour when the mod is off. */
	public static int outline(int vanilla) {
		if (!blockOverlay.value) return vanilla;
		return Theme.withAlpha(outlineColour.value, Math.round(outlineOpacity.get() * 2.55f));
	}

	// --- brightness ----------------------------------------------------------

	public static final Option.Bool brightness = new Option.Bool("brightness", "Brightness", false);
	public static final Option.Range brightnessLevel =
		new Option.Range("brightnessLevel", "Brightness", 300, 0, 1000, "%").step(10);

	/**
	 * The gamma the lightmap should be built with, or null to leave vanilla's.
	 * Fullbright wins when both are on: it is the brighter of the two.
	 */
	public static Double gammaOverride() {
		if (GameSettings.fullbrightEnabled()) return 1500.0D;
		if (brightness.value) return brightnessLevel.get() / 100.0D;
		return null;
	}

	// --- colour saturation ---------------------------------------------------

	public static final Option.Bool saturation = new Option.Bool("saturation", "Colour saturation", false);
	public static final Option.Range saturationLevel =
		new Option.Range("saturationLevel", "Saturation", 150, 0, 200, "%").step(10);

	/** The effect last asked for that never loaded, so a broken one is not retried every frame. */
	private static Identifier failedEffect;

	/**
	 * Runs once a frame before the world is drawn and keeps the game's post
	 * effect slot holding the chain for the chosen saturation.
	 *
	 * That slot is the one vanilla uses for the creeper and spider views while
	 * spectating, so the chain only goes in while the camera is the player's
	 * own - and is taken back out, never anything vanilla put there.
	 */
	public static void tickSaturation(GameRenderer renderer) {
		MinecraftClient mc = MinecraftClient.getInstance();
		Identifier want = null;
		if (saturation.value && mc.world != null && mc.player != null && mc.getCameraEntity() == mc.player
				&& saturationLevel.get() != 100) {
			want = Compat.saturationEffect(saturationLevel.get());
		}
		Identifier current = Compat.postEffect(renderer);
		if (want != null) {
			if (want.equals(current) || want.equals(failedEffect)) return;
			// Asking for a chain the game cannot find is a crash from 1.21.2 on,
			// not a log line, so never ask for one this jar does not carry.
			if (!BundledAssets.hasChain(want)) return;
			Compat.setPostEffect(renderer, want);
			failedEffect = Compat.postEffect(renderer) == null ? want : null;
		} else if (current != null && current.getNamespace().equals("mangoconfig")) {
			Compat.clearPostEffect(renderer);
		}
	}

	// --- custom fog ----------------------------------------------------------

	public static final Option.Bool customFog = new Option.Bool("customFog", "Custom fog", false);
	public static final Option.Bool noWorldFog = new Option.Bool("noWorldFog", "Remove world fog", true);
	public static final Option.Range fogDistance =
		new Option.Range("fogDistance", "Fog distance", 100, 25, 300, "%").step(5);
	public static final Option.Bool noLiquidFog = new Option.Bool("noLiquidFog", "Clear water and lava", false);

	/** Far enough that no fog is ever reached. */
	private static final float NO_FOG = 1000f;

	/**
	 * What to multiply vanilla's fog start and end by; 1 leaves them be.
	 *
	 * Blindness and darkness are never touched: those are the game telling you
	 * something, and seeing through them on a server would be a cheat.
	 */
	public static float fogScale() {
		if (!customFog.value) return 1f;
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc.player == null) return 1f;
		if (mc.player.hasStatusEffect(StatusEffects.BLINDNESS) || mc.player.hasStatusEffect(StatusEffects.DARKNESS)) {
			return 1f;
		}
		// The enum moved package in 1.21; its constant names did not.
		String submersion = mc.gameRenderer.getCamera().getSubmersionType().name();
		if (!submersion.equals("NONE")) return noLiquidFog.value ? NO_FOG : 1f;
		return noWorldFog.value ? NO_FOG : fogDistance.get() / 100f;
	}

	// --- hitboxes ------------------------------------------------------------

	public static final Option.Bool hitboxes = new Option.Bool("hitboxes", "Hitboxes", false);
	public static final Option.Colour hitboxColour = new Option.Colour("hitboxColour", "Box colour", Theme.BRAND);
	public static final Option.Bool hitboxEye = new Option.Bool("hitboxEye", "Eye height line", true);
	public static final Option.Bool hitboxLook = new Option.Bool("hitboxLook", "Look direction", true);

	/**
	 * Recolours one line of an F3+B hitbox. Vanilla tells its lines apart by
	 * colour alone - white box, red eye line, blue look vector - and has done
	 * in every version, so that is what this goes by too. A hidden line is
	 * drawn fully transparent rather than skipped, which needs no knowledge of
	 * how each version batches its lines.
	 */
	public static int hitbox(int argb) {
		if (!hitboxes.value) return argb;
		int rgb = argb & 0x00FFFFFF;
		if (rgb == 0xFFFFFF) return (argb & 0xFF000000) | (hitboxColour.value & 0x00FFFFFF);
		if (rgb == 0xFF0000) return hitboxEye.value ? argb : rgb;
		if (rgb == 0x0000FF) return hitboxLook.value ? argb : rgb;
		return argb;
	}

	/** The same, for the versions that pass the colour as four floats. */
	public static float[] hitbox(float r, float g, float b, float a) {
		int in = (Math.round(a * 255) << 24) | (Math.round(r * 255) << 16) | (Math.round(g * 255) << 8)
			| Math.round(b * 255);
		int out = hitbox(in);
		if (out == in) return new float[]{r, g, b, a};
		return new float[]{((out >> 16) & 0xFF) / 255f, ((out >> 8) & 0xFF) / 255f, (out & 0xFF) / 255f,
			((out >>> 24) & 0xFF) / 255f};
	}

	// --- chat ----------------------------------------------------------------

	public static final Option.Bool smoothChat = new Option.Bool("smoothChat", "Smooth chat", false);
	public static final Option.Range smoothChatTime =
		new Option.Range("smoothChatTime", "Slide time", 150, 50, 500, " ms").step(10);

	private static long lastLineNanos;

	/** Called for every line that lands in the chat box. */
	public static void onChatLine() {
		if (smoothChat.value) lastLineNanos = System.nanoTime();
	}

	/**
	 * How far below its resting place the chat is drawn this frame: a full
	 * line the moment a message arrives, easing back to nothing.
	 */
	public static float chatOffset() {
		if (!smoothChat.value || lastLineNanos == 0L) return 0f;
		float t = (System.nanoTime() - lastLineNanos) / (smoothChatTime.get() * 1_000_000f);
		if (t >= 1f) return 0f;
		float eased = 1f - (1f - t) * (1f - t) * (1f - t);
		var options = MinecraftClient.getInstance().options;
		float line = 9f * (float) (options.getChatLineSpacing().getValue() + 1.0D);
		return (1f - eased) * line * options.getChatScale().getValue().floatValue();
	}

	// --- F3 ------------------------------------------------------------------

	public static final Option.Bool customF3 = new Option.Bool("customF3", "Custom F3", false);
	public static final Option.Range f3Background =
		new Option.Range("f3Background", "Background", 100, 0, 100, "%").step(5);
	public static final Option.Colour f3TextColour = new Option.Colour("f3TextColour", "Text colour", Theme.TEXT);
	public static final Option.Bool f3Shadow = new Option.Bool("f3Shadow", "Text shadow", true);

	// --- tab list and scoreboard ---------------------------------------------

	public static final Option.Bool tabScoreboard = new Option.Bool("tabScoreboard", "Tab and scoreboard", false);
	public static final Option.Range tabBackground =
		new Option.Range("tabBackground", "Tab list background", 0, 0, 100, "%").step(5);
	public static final Option.Range tabEntries =
		new Option.Range("tabEntries", "Tab list rows", 100, 0, 100, "%").step(5);
	public static final Option.Range scoreboardBackground =
		new Option.Range("scoreboardBackground", "Scoreboard background", 0, 0, 100, "%").step(5);

	/** What vanilla is drawing right now, for the fills and text that need recolouring. */
	public enum Scope { NONE, TAB, SCOREBOARD, F3 }

	public static Scope scope = Scope.NONE;

	/** Vanilla fills each tab row with this translucent white. */
	private static final int TAB_ROW = 0x20FFFFFF;

	/** A background fill, made sheerer by whichever mod owns what is being drawn. */
	public static int fill(int argb) {
		switch (scope) {
			case TAB:
				if (!tabScoreboard.value) return argb;
				return fade(argb, argb == TAB_ROW ? tabEntries.get() : tabBackground.get());
			case SCOREBOARD:
				return tabScoreboard.value ? fade(argb, scoreboardBackground.get()) : argb;
			case F3:
				return customF3.value ? fade(argb, f3Background.get()) : argb;
			default:
				return argb;
		}
	}

	public static int text(int argb) {
		if (scope != Scope.F3 || !customF3.value) return argb;
		return 0xFF000000 | (f3TextColour.value & 0x00FFFFFF);
	}

	public static boolean shadow(boolean shadow) {
		if (scope != Scope.F3 || !customF3.value) return shadow;
		return f3Shadow.value;
	}

	/** Scales the alpha by a percentage, where 100 leaves the colour as vanilla made it. */
	private static int fade(int argb, int percent) {
		int alpha = (argb >>> 24) & 0xFF;
		return Theme.withAlpha(argb, Math.round(alpha * percent / 100f));
	}

	// --- UI scale ------------------------------------------------------------

	public static final Option.Bool uiScale = new Option.Bool("uiScale", "Inventory scale", false);
	public static final Option.Range inventoryScale = new Option.Range("inventoryScale", "Inventory GUI scale", 2, 0, 6, "");

	private static boolean scaledLastScreen;

	/** The GUI scale setting a screen should be laid out with. */
	public static int guiScaleFor(Screen screen, int vanilla) {
		return uiScale.value && screen instanceof HandledScreen<?> ? inventoryScale.get() : vanilla;
	}

	/**
	 * Re-lays out the open screen if it wants a different scale from the one
	 * the window has: called whenever a screen opens or closes, so an inventory
	 * comes up at its own size and the game goes back to the normal one after.
	 * Does nothing at all unless the mod is on or it scaled the last screen.
	 */
	public static void applyGuiScale(MinecraftClient mc) {
		if (!uiScale.value && !scaledLastScreen) return;
		Window window = mc.getWindow();
		if (window == null || mc.options == null) return;
		int vanilla = mc.options.getGuiScale().getValue();
		int setting = guiScaleFor(mc.currentScreen, vanilla);
		scaledLastScreen = setting != vanilla;
		int scale = window.calculateScaleFactor(setting, mc.forcesUnicodeFont());
		if (scale == Compat.scaleFactor(window)) return;
		Compat.setScaleFactor(window, scale);
		if (mc.currentScreen != null) {
			Compat.resize(mc.currentScreen, mc, window.getScaledWidth(), window.getScaledHeight());
		}
	}

	static {
		blockOverlay.hint = "Recolours the outline around the block you look at.";
		brightness.hint = "Lights the world past vanilla's 100% without touching its setting.";
		saturation.hint = "Richer or more washed-out colours, HUD untouched.";
		saturationLevel.hint = "100% is vanilla; 0% is black and white.";
		noWorldFog.hint = "Off uses the distance below instead.";
		fogDistance.hint = "How far the fog sits, against vanilla's distance.";
		noLiquidFog.hint = "Blindness and darkness are always left alone.";
		hitboxes.hint = "Restyles the F3+B hitboxes.";
		smoothChat.hint = "New messages slide in instead of popping up.";
		f3Background.hint = "100% is vanilla; 0% drops the grey boxes.";
		tabBackground.hint = "100% is vanilla; 0% is fully see-through.";
		tabEntries.hint = "The light strip behind each name.";
		scoreboardBackground.hint = "100% is vanilla; 0% is fully see-through.";
		uiScale.hint = "Inventories and containers get their own GUI scale.";
		inventoryScale.hint = "0 means automatic; capped to what fits the window.";
	}

	/** Everything that must be written to the config file. */
	public static List<Option> all() {
		List<Option> list = new ArrayList<>();
		list.add(blockOverlay);
		list.add(outlineColour);
		list.add(outlineOpacity);
		list.add(brightness);
		list.add(brightnessLevel);
		list.add(saturation);
		list.add(saturationLevel);
		list.add(customFog);
		list.add(noWorldFog);
		list.add(fogDistance);
		list.add(noLiquidFog);
		list.add(hitboxes);
		list.add(hitboxColour);
		list.add(hitboxEye);
		list.add(hitboxLook);
		list.add(smoothChat);
		list.add(smoothChatTime);
		list.add(customF3);
		list.add(f3Background);
		list.add(f3TextColour);
		list.add(f3Shadow);
		list.add(tabScoreboard);
		list.add(tabBackground);
		list.add(tabEntries);
		list.add(scoreboardBackground);
		list.add(uiScale);
		list.add(inventoryScale);
		return list;
	}
}
