package com.notes.backend.common.security;

import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import java.io.IOException;
import java.util.List;
import java.util.UUID;
import org.springframework.http.HttpHeaders;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

@Component
public class JwtAuthFilter extends OncePerRequestFilter {
    private final JwtProvider jwtProvider;
    private final DeviceStatusChecker deviceStatusChecker;

    public JwtAuthFilter(JwtProvider jwtProvider, DeviceStatusChecker deviceStatusChecker) {
        this.jwtProvider = jwtProvider;
        this.deviceStatusChecker = deviceStatusChecker;
    }

    @Override
    protected void doFilterInternal(
        HttpServletRequest request,
        HttpServletResponse response,
        FilterChain filterChain
    ) throws ServletException, IOException {
        String header = request.getHeader(HttpHeaders.AUTHORIZATION);
        String deviceHeader = request.getHeader("X-Device-Id");
        if (header != null && header.startsWith("Bearer ")) {
            AuthPrincipal principal = jwtProvider.parseAccessToken(header.substring(7));
            if (principal != null && matchesDeviceHeader(principal.deviceId(), deviceHeader)
                && deviceStatusChecker.isActiveDevice(principal.userId(), principal.deviceId())) {
                var auth = new UsernamePasswordAuthenticationToken(principal, null, List.of());
                SecurityContextHolder.getContext().setAuthentication(auth);
            }
        }
        filterChain.doFilter(request, response);
    }

    private static boolean matchesDeviceHeader(UUID tokenDeviceId, String deviceHeader) {
        if (deviceHeader == null || deviceHeader.isBlank()) {
            return false;
        }
        try {
            return tokenDeviceId.equals(UUID.fromString(deviceHeader.trim()));
        } catch (IllegalArgumentException ex) {
            return false;
        }
    }
}
