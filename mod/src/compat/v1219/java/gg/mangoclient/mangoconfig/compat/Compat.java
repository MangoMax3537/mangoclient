package gg.mangoclient.mangoconfig.compat;

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
		return null;
	}
	public static boolean isKeyPressed(Window window, int key) {
		return InputUtil.isKeyPressed(window, key);
	}
	public static void drawGuiTexture(DrawContext ctx, Identifier sprite, int x, int y, int w, int h) {
		ctx.drawGuiTexture(RenderPipelines.GUI_TEXTURED, sprite, x, y, w, h);
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
}
