package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Mods;
import gg.mangoclient.mangoconfig.Pvp;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The two things that need the camera itself: zooming and holding the view
 * still when the player is hit.
 *
 * Both read a flag and nothing else, so a frame costs the same whether the mod
 * is on or off.
 */
@Mixin(GameRenderer.class)
public class GameRendererMixin {
	@Inject(method = "getFov", at = @At("RETURN"), cancellable = true)
	private void mangoconfig$zoom(Camera camera, float tickDelta, boolean changingFov,
			CallbackInfoReturnable<Double> cir) {
		float factor = Mods.zoomFactor();
		if (factor > 1.0001f) cir.setReturnValue(cir.getReturnValue() / factor);
	}

	@Inject(method = "tiltViewWhenHurt", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$noHurtCam(MatrixStack matrices, float tickDelta, CallbackInfo ci) {
		if (Mods.noHurtCam.value) ci.cancel();
	}

	// The totem flash is the item vanilla floats across the screen; never
	// handing it over means no flash, while the sound and particles stay.
	@Inject(method = "showFloatingItem", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$noTotemPop(ItemStack stack, CallbackInfo ci) {
		if (Pvp.noTotemAnimation.value && stack.isOf(Items.TOTEM_OF_UNDYING)) ci.cancel();
	}
}
