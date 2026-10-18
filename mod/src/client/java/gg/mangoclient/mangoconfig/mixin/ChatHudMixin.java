package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.client.gui.hud.ChatHudLine;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Smooth chat: notes when a line arrives, so the HUD can slide it in. */
@Mixin(ChatHud.class)
public class ChatHudMixin {
	@Inject(method = "addVisibleMessage", at = @At("HEAD"))
	private void mangoconfig$newLine(ChatHudLine message, CallbackInfo ci) {
		Visuals.onChatLine();
	}
}
