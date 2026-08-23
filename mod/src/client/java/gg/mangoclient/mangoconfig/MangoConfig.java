package gg.mangoclient.mangoconfig;

import com.google.gson.JsonObject;
import gg.mangoclient.mangoconfig.modules.*;
import net.fabricmc.api.ClientModInitializer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.util.InputUtil;

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
	private static final List<Category> CATEGORIES = new ArrayList<>();
	private static ConfigFile config;

	/** Rising-edge state for the open key, so holding it does not toggle madly. */
	private static boolean openKeyWasDown;

	@Override
	public void onInitializeClient() {
		// Roughly in the order a player would look for them: performance, then
		// the world, then the fight, then the odds and ends.
		MODULES.add(new FpsModule());
		MODULES.add(new PingModule());
		MODULES.add(new MemoryModule());
		MODULES.add(new CoordinatesModule());
		MODULES.add(new DirectionModule());
		MODULES.add(new BiomeModule());
		MODULES.add(new DayModule());
		MODULES.add(new ServerModule());
		MODULES.add(new SpeedModule());
		MODULES.add(new ArmourModule());
		MODULES.add(new StatsModule());
		MODULES.add(new SaturationModule());
		MODULES.add(new PotionsModule());
		MODULES.add(new ItemsModule());
		MODULES.add(new CpsModule());
		MODULES.add(new ReachModule());
		MODULES.add(new ComboModule());
		MODULES.add(new KeystrokesModule());
		MODULES.add(new SprintModule());
		MODULES.add(new SessionModule());
		MODULES.add(new ClockModule());

		config = new ConfigFile();
		config.load(MODULES);

		Presence.init();
	}

	public static List<HudModule> modules() {
		return MODULES;
	}

	/**
	 * Built on first use rather than at start-up: Minecraft's own options do
	 * not exist yet while mods are initialising, and half this screen is bound
	 * straight to them.
	 */
	public static List<Category> categories() {
		if (CATEGORIES.isEmpty()) {
			// One "Mods" page carries the HUD modules and the standalone mods
			// alike; PvP keeps everything a fighter reaches for in one place.
			List<ModEntry> mods = new ArrayList<>(MODULES.stream().map(ModEntry::of).toList());
			mods.add(ModEntry.of("Zoom", "Hold a key to zoom the view in.",
				Mods.zoom, Mods.zoomKey, Mods.zoomLevel));
			mods.add(ModEntry.of("Crosshair", "Replaces vanilla's crosshair with your own.",
				Mods.crosshair, Mods.crosshairStyle, Mods.crosshairWidth, Mods.crosshairHeight,
				Mods.crosshairGap, Mods.crosshairThickness, Mods.crosshairColour, Mods.crosshairOutline,
				Mods.crosshairHideThirdPerson));
			mods.add(ModEntry.of("Fullbright", "Pins brightness to its maximum.",
				GameSettings.fullbright()));
			mods.add(ModEntry.of("Mango Badge", "A grey mango next to everyone on MangoClient.",
				Mods.mangoBadge));
			CATEGORIES.add(new Category("mods", "Mods", "The HUD and everything else the client adds.")
				.withEntries(mods));
			CATEGORIES.add(new Category("pvp", "PvP", "Client-side tweaks for the fight.")
				.withEntries(List.of(
					ModEntry.of("Lower Shield", "Blocks with the shield held lower on screen.",
						Pvp.lowerShield, Pvp.lowerShieldAmount),
					ModEntry.of("No Explosions", "Hides crystal, TNT and wind charge explosions.",
						Pvp.noExplosions),
					ModEntry.of("No Totem Pop", "Skips the totem flash across the screen.",
						Pvp.noTotemAnimation),
					ModEntry.of("No Hurt Cam", "Keeps the view still when something hits you.",
						Mods.noHurtCam))));
			CATEGORIES.add(new Category("overlays", "Overlays", "Parts of the vanilla HUD you can drop.")
				.withEntries(List.of(
					ModEntry.of("Hide Scoreboard", "Drops the scoreboard on the right.", Mods.hideScoreboard),
					ModEntry.of("Hide Boss Bar", "Drops the boss bar at the top.", Mods.hideBossBar),
					ModEntry.of("Hide Effects", "Drops vanilla's effect icons.", Mods.hideEffectIcons),
					ModEntry.of("Hide Portal", "Drops the purple haze in a portal.", Mods.hidePortalOverlay),
					ModEntry.of("Hide Nausea", "Drops the nausea wobble.", Mods.hideNausea),
					ModEntry.of("Hide Item Name", "Drops the item name popup.", Mods.hideItemName))));
			CATEGORIES.add(new Category("video", "Video", "Minecraft's own video settings.")
				.withOptions(GameSettings.video()));
			CATEGORIES.add(new Category("controls", "Controls", "Mouse, keys and chat.")
				.withOptions(GameSettings.controls()));
			CATEGORIES.add(new Category("client", "Client", "MangoConfig itself.")
				.withOption(new Option.Bool("hudEnabled", "Show HUD", true)
					.bind(() -> config.hudEnabled, v -> { config.hudEnabled = v; save(); }))
				.withOption(Mods.openKey)
				.withOption(new Option.Readout("Active modules",
					() -> MODULES.stream().filter(m -> m.enabled).count() + " of " + MODULES.size())));
		}
		return CATEGORIES;
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
		pollZoomKey(mc);
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
			// The editor arranges what is actually switched on; drawing the rest
			// would bury the screen under boxes nobody asked for.
			if (!module.enabled) continue;
			if (!preview && !module.visible(mc)) continue;
			drawModule(ctx, mc, font, module, screenW, screenH, preview);
		}
	}

	/**
	 * True while the HUD editor is open. Modules that would draw nothing use it
	 * to show a sample instead, so there is something to take hold of.
	 */
	public static boolean previewing() {
		MinecraftClient mc = MinecraftClient.getInstance();
		return mc != null && mc.currentScreen instanceof HudEditorScreen;
	}

	public static void drawModule(DrawContext ctx, MinecraftClient mc, TextRenderer font,
			HudModule module, int screenW, int screenH, boolean preview) {
		// Position and clamping come from bounds(), the same rectangle the HUD
		// editor frames - so what is dragged there is exactly what draws here,
		// padding included, instead of two slightly different boxes.
		float s = module.scaleFactor();
		int w = Math.max(1, module.width(mc, font));
		int h = Math.max(1, module.height(mc, font));
		int[] b = bounds(mc, module);
		int inset = Math.round(module.padding() * s);

		ctx.getMatrices().pushMatrix();
		ctx.getMatrices().translate((float) (b[0] + inset), (float) (b[1] + inset));
		ctx.getMatrices().scale(s, s);
		module.renderBackground(ctx, w, h);
		module.render(ctx, mc, font, preview);
		ctx.getMatrices().popMatrix();
	}

	private static void pollOpenKey(MinecraftClient mc) {
		if (config == null) return;
		boolean down = InputUtil.isKeyPressed(mc.getWindow(), Mods.openKey.value);
		if (down && !openKeyWasDown && mc.currentScreen == null) {
			mc.setScreen(new MangoConfigScreen());
		}
		openKeyWasDown = down;
	}

	/**
	 * Zoom is held rather than toggled, and only in the world: holding the key
	 * while a menu is open would zoom the moment the menu closed.
	 */
	private static void pollZoomKey(MinecraftClient mc) {
		Mods.zoomActive = Mods.zoom.value
			&& mc.currentScreen == null
			&& InputUtil.isKeyPressed(mc.getWindow(), Mods.zoomKey.value);
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

}
