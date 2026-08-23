package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Pvp;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.command.OrderedRenderCommandQueue;
import net.minecraft.client.render.item.HeldItemRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.util.Hand;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Lower Shield: the first-person shield sits further down while blocking.
 *
 * Vanilla already lowers a hand by its "equip progress" while items swap;
 * adding to that number for a blocking shield reuses the exact translation
 * vanilla applies, so nothing new is pushed onto any matrix and the animation
 * in and out of blocking stays vanilla's own.
 */
@Mixin(HeldItemRenderer.class)
public class HeldItemRendererMixin {
	@ModifyVariable(method = "renderFirstPersonItem", at = @At("HEAD"), ordinal = 3, argsOnly = true)
	private float mangoconfig$lowerShield(float equipProgress, AbstractClientPlayerEntity player, float tickDelta,
			float pitch, Hand hand, float swingProgress, ItemStack item, float equipProgressArg,
			MatrixStack matrices, OrderedRenderCommandQueue queue, int light) {
		if (item.isOf(Items.SHIELD) && player.isUsingItem() && player.getActiveHand() == hand) {
			return equipProgress + Pvp.shieldOffset();
		}
		return equipProgress;
	}
}
