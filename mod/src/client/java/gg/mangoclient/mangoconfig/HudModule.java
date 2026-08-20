package gg.mangoclient.mangoconfig;

import com.google.gson.JsonObject;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.ArrayList;
import java.util.List;

/**
 * One thing drawn on the HUD.
 *
 * Position is kept as a fraction of the screen rather than pixels, so an
 * element parked in a corner stays in that corner when the window is resized
 * or the GUI scale changes.
 */
public abstract class HudModule {
	public final String id;
	public final String name;
	public final String description;

	public boolean enabled;
	public float x;
	public float y;

	/** Settings every module has. Module-specific ones go in {@link #options}. */
	public final Option.Range scale = new Option.Range("scale", "Größe", 100, 50, 200, "%");
	public final Option.Colour colour = new Option.Colour("colour", "Textfarbe", Theme.TEXT);
	public final Option.Bool shadow = new Option.Bool("shadow", "Schatten", true);
	public final Option.Bool background = new Option.Bool("background", "Hintergrund", true);
	public final Option.Colour backgroundColour = new Option.Colour("bgColour", "Hintergrundfarbe", Theme.HUD_BG);

	public final List<Option> options = new ArrayList<>();

	protected HudModule(String id, String name, String description, boolean enabled, float x, float y) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.enabled = enabled;
		this.x = x;
		this.y = y;
	}

	/** Unscaled width of what {@link #render} draws. */
	public abstract int width(MinecraftClient mc, TextRenderer font);

	/** Unscaled height of what {@link #render} draws. */
	public abstract int height(MinecraftClient mc, TextRenderer font);

	/**
	 * Draw at 0,0; the caller has already translated and scaled.
	 *
	 * `preview` is true in the editor, where there may be no server to ask for
	 * a ping and no armour to show. A module that would otherwise draw nothing
	 * has to draw something there, or it cannot be dragged.
	 */
	public abstract void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview);

	public int padding() {
		return background.value ? 3 : 0;
	}

	public float scaleFactor() {
		return scale.value / 100f;
	}

	public int pixelX(int screenW, int scaledW) {
		return Math.round(Math.max(0, Math.min(screenW - scaledW, x * screenW)));
	}

	public int pixelY(int screenH, int scaledH) {
		return Math.round(Math.max(0, Math.min(screenH - scaledH, y * screenH)));
	}

	public void renderBackground(DrawContext ctx, int w, int h) {
		if (!background.value) return;
		int pad = padding();
		ctx.fill(-pad, -pad, w + pad, h + pad, backgroundColour.value);
	}

	protected void text(DrawContext ctx, TextRenderer font, String s, int x, int y) {
		ctx.drawText(font, s, x, y, colour.value, shadow.value);
	}

	public void save(JsonObject json) {
		json.addProperty("enabled", enabled);
		json.addProperty("x", x);
		json.addProperty("y", y);
		scale.save(json);
		colour.save(json);
		shadow.save(json);
		background.save(json);
		backgroundColour.save(json);
		for (Option option : options) option.save(json);
	}

	public void load(JsonObject json) {
		if (json.has("enabled")) enabled = json.get("enabled").getAsBoolean();
		if (json.has("x")) x = clamp01(json.get("x").getAsFloat());
		if (json.has("y")) y = clamp01(json.get("y").getAsFloat());
		scale.load(json);
		colour.load(json);
		shadow.load(json);
		background.load(json);
		backgroundColour.load(json);
		for (Option option : options) option.load(json);
	}

	private static float clamp01(float v) {
		return Math.max(0f, Math.min(1f, v));
	}
}
