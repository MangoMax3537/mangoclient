package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import gg.mangoclient.mangoconfig.compat.Compat;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Smooth chat: the whole chat box is drawn a little low while a new line
 * arrives and eases back up, which reads as the line sliding in.
 */
@Mixin(InGameHud.class)
public class ChatSlideMixin {
	@Unique private boolean mangoconfig$chatShifted;

	@Inject(method = "renderChat", at = @At("HEAD"))
	private void mangoconfig$slideEnter(DrawContext context, float tickDelta, CallbackInfo ci) {
		float offset = Visuals.chatOffset();
		mangoconfig$chatShifted = offset > 0f;
		if (!mangoconfig$chatShifted) return;
		Compat.push(context);
		Compat.translate(context, 0f, offset);
	}

	@Inject(method = "renderChat", at = @At("RETURN"))
	private void mangoconfig$slideLeave(DrawContext context, float tickDelta, CallbackInfo ci) {
		if (!mangoconfig$chatShifted) return;
		mangoconfig$chatShifted = false;
		Compat.pop(context);
	}
}
