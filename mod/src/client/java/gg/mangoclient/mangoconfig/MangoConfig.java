package gg.mangoclient.mangoconfig;

import com.google.gson.JsonObject;
import gg.mangoclient.mangoconfig.modules.*;
import net.fabricmc.api.ClientModInitializer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/**
 * MangoConfig - MangoClient's in-game HUD and settings layer.
 *
 * Everything runs off the one HUD hook in GuiMixin: the modules are drawn, the
 * open key is polled, and the click counter is sampled. No Fabric API, no
 * lifecycle events, nothing else to go wrong on a stranger's instance.
 */
public class MangoConfig implements ClientModInitializer {
	public static final String NAME = "MangoConfig";

	private static final List<HudModule> MODULES = new ArrayList<>();
	private static ConfigFile config;

	/** Rising-edge state for the open key, so holding it does not toggle madly. */
	private static boolean openKeyWasDown;

	@Override
	public void onInitializeClient() {
		MODULES.add(new ArmourModule());
		MODULES.add(new CoordinatesModule());
		MODULES.add(new FpsModule());
		MODULES.add(new PingModule());
		MODULES.add(new CpsModule());
		MODULES.add(new KeystrokesModule());
		MODULES.add(new PotionsModule());
		MODULES.add(new ClockModule());

		config = new ConfigFile();
		config.load(MODULES);
	}

	public static List<HudModule> modules() {
		return MODULES;
	}

	public static ConfigFile config() {
		return config;
	}

	public static void save() {
		if (config != null) config.save(MODULES);
	}

	/** Called from the HUD mixin, after vanilla has drawn its own overlay. */
	public static void onHudRender(DrawContext ctx) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc == null || mc.player == null) return;

		pollOpenKey(mc);
		ClickCounter.tick();

		// The editor draws the modules itself, with handles; drawing them here
		// too would double every element.
		if (mc.currentScreen instanceof HudEditorScreen) return;
		if (config != null && !config.hudEnabled) return;
		renderModules(ctx, mc, false);
	}

	/** Shared by the HUD and the editor so what you drag is what you get. */
	public static void renderModules(DrawContext ctx, MinecraftClient mc, boolean preview) {
		TextRenderer font = mc.textRenderer;
		int screenW = mc.getWindow().getScaledWidth();
		int screenH = mc.getWindow().getScaledHeight();

		for (HudModule module : MODULES) {
			if (!module.enabled && !preview) continue;
			drawModule(ctx, mc, font, module, screenW, screenH, preview);
		}
	}

	public static void drawModule(DrawContext ctx, MinecraftClient mc, TextRenderer font,
			HudModule module, int screenW, int screenH, boolean preview) {
		float s = module.scaleFactor();
		int w = Math.max(1, module.width(mc, font));
		int h = Math.max(1, module.height(mc, font));
		int px = module.pixelX(screenW, Math.round(w * s));
		int py = module.pixelY(screenH, Math.round(h * s));

		ctx.getMatrices().pushMatrix();
		ctx.getMatrices().translate((float) px, (float) py);
		ctx.getMatrices().scale(s, s);
		module.renderBackground(ctx, w, h);
		module.render(ctx, mc, font, preview);
		ctx.getMatrices().popMatrix();
	}

	private static void pollOpenKey(MinecraftClient mc) {
		if (config == null) return;
		boolean down = InputUtil.isKeyPressed(mc.getWindow(), config.openKey);
		if (down && !openKeyWasDown && mc.currentScreen == null) {
			mc.setScreen(new MangoConfigScreen());
		}
		openKeyWasDown = down;
	}

	/** Bounds of a module on screen, used by the editor for hit testing. */
	public static int[] bounds(MinecraftClient mc, HudModule module) {
		TextRenderer font = mc.textRenderer;
		int screenW = mc.getWindow().getScaledWidth();
		int screenH = mc.getWindow().getScaledHeight();
		float s = module.scaleFactor();
		int pad = module.padding();
		int w = Math.round((Math.max(1, module.width(mc, font)) + pad * 2) * s);
		int h = Math.round((Math.max(1, module.height(mc, font)) + pad * 2) * s);
		return new int[]{module.pixelX(screenW, w), module.pixelY(screenH, h), w, h};
	}

	/** Everything a module owns, as JSON, for the config file. */
	public static JsonObject dump(HudModule module) {
		JsonObject json = new JsonObject();
		module.save(json);
		return json;
	}

	public static int defaultOpenKey() {
		return GLFW.GLFW_KEY_RIGHT_SHIFT;
	}
}
