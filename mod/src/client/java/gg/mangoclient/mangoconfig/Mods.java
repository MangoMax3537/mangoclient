package gg.mangoclient.mangoconfig;

import net.minecraft.client.gui.DrawContext;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/**
 * The mods that are not HUD elements: things that change how the game looks or
 * behaves rather than drawing a plate somewhere.
 *
 * They live here as plain settings so the screen can draw them with the same
 * rows it draws everything else, and the mixins only ever read a boolean.
 */
public final class Mods {
	private Mods() {
	}

	/** The key that opens the settings screen; shown on the Client page. */
	public static final Option.Key openKey =
		new Option.Key("openKey", "Open MangoConfig", GLFW.GLFW_KEY_RIGHT_SHIFT);

	// --- zoom ---------------------------------------------------------------

	public static final Option.Bool zoom = new Option.Bool("zoom", "Zoom", true);
	public static final Option.Range zoomLevel = new Option.Range("zoomLevel", "Zoom amount", 4, 2, 10, "x");
	public static final Option.Key zoomKey = new Option.Key("zoomKey", "Zoom key", GLFW.GLFW_KEY_C);

	/** Set once a frame from the HUD hook; the renderer mixin only reads it. */
	public static volatile boolean zoomActive;

	public static float zoomFactor() {
		return zoomLevel.value;
	}

	// --- camera and crosshair ------------------------------------------------

	public static final Option.Bool noHurtCam = new Option.Bool("noHurtCam", "No hurt camera", false);

	public static final Option.Bool crosshair = new Option.Bool("crosshair", "Custom crosshair", false);
	public static final Option.Choice crosshairStyle =
		new Option.Choice("crosshairStyle", "Crosshair style", 0, "Cross", "Dot", "Cross + dot");
	public static final Option.Range crosshairWidth =
		new Option.Range("crosshairWidth", "Arm length - across", 4, 0, 16, " px");
	public static final Option.Range crosshairHeight =
		new Option.Range("crosshairHeight", "Arm length - up/down", 4, 0, 16, " px");
	public static final Option.Range crosshairGap = new Option.Range("crosshairGap", "Centre gap", 2, 0, 8, " px");
	public static final Option.Range crosshairThickness =
		new Option.Range("crosshairThickness", "Thickness", 1, 1, 4, " px");
	public static final Option.Colour crosshairColour = new Option.Colour("crosshairColour", "Colour", Theme.TEXT);
	public static final Option.Bool crosshairOutline = new Option.Bool("crosshairOutline", "Outline", true);
	public static final Option.Bool crosshairHideThirdPerson =
		new Option.Bool("crosshairHideThirdPerson", "Hide in third person", true);

	// --- overlays vanilla draws that a player may not want -------------------

	public static final Option.Bool hideScoreboard = new Option.Bool("hideScoreboard", "Hide scoreboard", false);
	public static final Option.Bool hideBossBar = new Option.Bool("hideBossBar", "Hide boss bar", false);
	public static final Option.Bool hideEffectIcons =
		new Option.Bool("hideEffectIcons", "Hide vanilla effect icons", false);
	public static final Option.Bool hidePortalOverlay =
		new Option.Bool("hidePortalOverlay", "Hide portal overlay", false);
	public static final Option.Bool hideNausea = new Option.Bool("hideNausea", "Hide nausea wobble", false);
	public static final Option.Bool hideItemName = new Option.Bool("hideItemName", "Hide item name popup", false);

	static {
		zoom.hint = "Hold the key to zoom in.";
		noHurtCam.hint = "Keeps the view still when something hits you.";
		crosshair.hint = "Replaces vanilla's, and its attack indicator with it.";
		crosshairWidth.hint = "The left and right arms; 0 drops them.";
		crosshairHeight.hint = "The top and bottom arms; 0 drops them.";
		crosshairHideThirdPerson.hint = "No crosshair while the camera is behind you (F5).";
		hideEffectIcons.hint = "The Effects module can show them instead.";
		hidePortalOverlay.hint = "The purple haze while standing in a portal.";
		hideItemName.hint = "The name that flashes up when you swap item.";
	}

	/** Everything that must be written to the config file. */
	public static List<Option> all() {
		List<Option> list = new ArrayList<>(gameplay());
		list.addAll(overlays());
		list.add(openKey);
		return list;
	}

	public static List<Option> gameplay() {
		List<Option> list = new ArrayList<>();
		list.add(zoom);
		list.add(zoomKey);
		list.add(zoomLevel);
		list.add(noHurtCam);
		list.add(crosshair);
		list.add(crosshairStyle);
		list.add(crosshairWidth);
		list.add(crosshairHeight);
		list.add(crosshairGap);
		list.add(crosshairThickness);
		list.add(crosshairColour);
		list.add(crosshairOutline);
		list.add(crosshairHideThirdPerson);
		return list;
	}

	public static List<Option> overlays() {
		List<Option> list = new ArrayList<>();
		list.add(hideScoreboard);
		list.add(hideBossBar);
		list.add(hideEffectIcons);
		list.add(hidePortalOverlay);
		list.add(hideNausea);
		list.add(hideItemName);
		return list;
	}

	// --- crosshair drawing ---------------------------------------------------

	/**
	 * Drawn in place of vanilla's, dead centre of the screen.
	 *
	 * Everything is whole pixels in GUI space: a crosshair that lands between
	 * two pixels blurs, and a blurred crosshair is worse than vanilla's.
	 */
	public static void renderCrosshair(DrawContext ctx) {
		int cx = ctx.getScaledWindowWidth() / 2;
		int cy = ctx.getScaledWindowHeight() / 2;

		int t = crosshairThickness.value;
		int half = t / 2;
		int w = crosshairWidth.value;
		int h = crosshairHeight.value;
		int gap = crosshairGap.value;
		int colour = crosshairColour.value;
		boolean dot = crosshairStyle.is("Dot") || crosshairStyle.is("Cross + dot");
		boolean arms = !crosshairStyle.is("Dot");

		// The two axes are sized on their own, so a wide, flat crosshair - or
		// a single horizontal line at height 0 - is a slider away.
		if (arms) {
			bar(ctx, cx - gap - w, cy - half, w, t, colour);
			bar(ctx, cx + gap + t - half, cy - half, w, t, colour);
			bar(ctx, cx - half, cy - gap - h, t, h, colour);
			bar(ctx, cx - half, cy + gap + t - half, t, h, colour);
		}
		if (dot) {
			bar(ctx, cx - half, cy - half, t, t, colour);
		}
	}

	private static void bar(DrawContext ctx, int x, int y, int w, int h, int colour) {
		if (w <= 0 || h <= 0) return;
		if (crosshairOutline.value) {
			ctx.fill(x - 1, y - 1, x + w + 1, y + h + 1, 0xC0000000);
		}
		ctx.fill(x, y, x + w, y + h, colour);
	}
}
