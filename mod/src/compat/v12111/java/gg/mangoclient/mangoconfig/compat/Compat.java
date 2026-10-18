package gg.mangoclient.mangoconfig.compat;

import gg.mangoclient.mangoconfig.mixin.GameRendererAccessor;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.GameRenderer;
import com.mojang.authlib.GameProfile;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.item.ItemStack;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gl.RenderPipelines;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.option.SimpleOption;
import net.minecraft.client.util.InputUtil;
import net.minecraft.client.util.Window;
import net.minecraft.text.Style;
import net.minecraft.text.StyleSpriteSource;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.UUID;

/**
 * The 1.21.9+ face of the small set of APIs that drift between Minecraft
 * versions. The build picks one of the src/compat trees; everything else
 * compiles against whichever face is chosen.
 */
public final class Compat {
	private Compat() {
	}

	public static Style withFont(Style style, Identifier font) {
		return style.withFont(new StyleSpriteSource.Font(font));
	}

	public static UUID profileId(GameProfile profile) {
		return profile.id();
	}

	public static void push(DrawContext ctx) {
		ctx.getMatrices().pushMatrix();
	}

	public static void pop(DrawContext ctx) {
		ctx.getMatrices().popMatrix();
	}

	public static void translate(DrawContext ctx, float x, float y) {
		ctx.getMatrices().translate(x, y);
	}

	public static void scale(DrawContext ctx, float sx, float sy) {
		ctx.getMatrices().scale(sx, sy);
	}
	public static SimpleOption<Boolean> vignette(GameOptions options) {
		return options.getVignette();
	}
	public static boolean isKeyPressed(Window window, int key) {
		return InputUtil.isKeyPressed(window, key);
	}
	public static void drawGuiTexture(DrawContext ctx, Identifier sprite, int x, int y, int w, int h) {
		ctx.drawGuiTexture(RenderPipelines.GUI_TEXTURED, sprite, x, y, w, h);
	}
	public static void drawTexture(DrawContext ctx, Identifier texture, int x, int y, int w, int h) {
		ctx.drawTexture(RenderPipelines.GUI_TEXTURED, texture, x, y, 0f, 0f, w, h, w, h);
	}
	public static List<ItemStack> mainStacks(PlayerInventory inventory) {
		return inventory.getMainStacks();
	}
	public static void drawStackOverlay(DrawContext ctx, TextRenderer font, ItemStack stack, int x, int y) {
		ctx.drawStackOverlay(font, stack, x, y);
	}
	public static Identifier vanillaId(String path) {
		return Identifier.ofVanilla(path);
	}

	public static int scaleFactor(Window window) {
		return window.getScaleFactor();
	}

	public static void setScaleFactor(Window window, int scale) {
		window.setScaleFactor(scale);
	}

	public static void resize(Screen screen, MinecraftClient client, int width, int height) {
		screen.resize(width, height);
	}

	/** The post chain for a saturation step, by its post_effect id. */
	public static Identifier saturationEffect(int percent) {
		return Identifier.of("mangoconfig", "saturation_" + pad(percent));
	}

	public static Identifier postEffect(GameRenderer renderer) {
		return renderer.getPostProcessorId();
	}

	public static void setPostEffect(GameRenderer renderer, Identifier id) {
		((GameRendererAccessor) renderer).mangoconfig$setPostProcessor(id);
	}

	public static void clearPostEffect(GameRenderer renderer) {
		renderer.clearPostProcessor();
	}

	private static String pad(int percent) {
		return String.format(java.util.Locale.ROOT, "%03d", percent);
	}

	/** The signed-in player's face, hat layer included, for the home screen. */
	public static void drawFace(DrawContext ctx, MinecraftClient client, int x, int y, int size) {
		net.minecraft.client.gui.PlayerSkinDrawer.draw(ctx, client.getSkinProvider().supplySkinTextures(client.getGameProfile(), false).get(), x, y, size);
	}

	/** The vanilla button click, for screens that draw their own buttons. */
	public static void clickSound(MinecraftClient client) {
		client.getSoundManager().play(net.minecraft.client.sound.PositionedSoundInstance.ui(
			net.minecraft.sound.SoundEvents.UI_BUTTON_CLICK, 1.0f));
	}
}
