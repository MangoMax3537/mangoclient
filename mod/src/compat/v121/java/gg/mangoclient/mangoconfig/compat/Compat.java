package gg.mangoclient.mangoconfig.compat;

import com.mojang.authlib.GameProfile;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.item.ItemStack;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.option.SimpleOption;
import net.minecraft.client.util.InputUtil;
import net.minecraft.client.util.Window;
import net.minecraft.text.Style;
import net.minecraft.util.Identifier;

import java.util.List;
import java.util.UUID;

/**
 * The 1.21-1.21.1 face: fonts go by Identifier, authlib says getId(), and
 * DrawContext still carries the 3D MatrixStack, so the 2D calls pick up a z.
 */
public final class Compat {
	private Compat() {
	}

	public static Style withFont(Style style, Identifier font) {
		return style.withFont(font);
	}

	public static UUID profileId(GameProfile profile) {
		return profile.getId();
	}

	public static void push(DrawContext ctx) {
		ctx.getMatrices().push();
	}

	public static void pop(DrawContext ctx) {
		ctx.getMatrices().pop();
	}

	public static void translate(DrawContext ctx, float x, float y) {
		ctx.getMatrices().translate(x, y, 0);
	}

	public static void scale(DrawContext ctx, float sx, float sy) {
		ctx.getMatrices().scale(sx, sy, 1);
	}
	public static SimpleOption<Boolean> vignette(GameOptions options) {
		return null;
	}
	public static boolean isKeyPressed(Window window, int key) {
		return InputUtil.isKeyPressed(window.getHandle(), key);
	}
	public static void drawGuiTexture(DrawContext ctx, Identifier sprite, int x, int y, int w, int h) {
		ctx.drawGuiTexture(sprite, x, y, w, h);
	}
	public static List<ItemStack> mainStacks(PlayerInventory inventory) {
		return inventory.main;
	}
	public static void drawStackOverlay(DrawContext ctx, TextRenderer font, ItemStack stack, int x, int y) {
		ctx.drawItemInSlot(font, stack, x, y);
	}
	public static Identifier vanillaId(String path) {
		return Identifier.ofVanilla(path);
	}
}
