package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Ping;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.network.packet.s2c.query.PingResultS2CPacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The other half of the ping measurement: the server's echo coming back.
 *
 * Vanilla hands this to the debug chart's measurer and nothing else, so the
 * value is thrown away unless F3 is open. Reading it at the head costs a field
 * write and leaves vanilla's own handling exactly as it was.
 */
@Mixin(ClientPlayNetworkHandler.class)
public class ClientPlayNetworkHandlerMixin {
	@Inject(method = "onPingResult", at = @At("HEAD"))
	private void mangoconfig$onPingResult(PingResultS2CPacket packet, CallbackInfo ci) {
		Ping.onResult(packet.startTime());
	}
}
