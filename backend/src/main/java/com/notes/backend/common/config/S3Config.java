package com.notes.backend.common.config;

import java.net.URI;
import org.springframework.boot.ApplicationRunner;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import software.amazon.awssdk.auth.credentials.AwsBasicCredentials;
import software.amazon.awssdk.auth.credentials.StaticCredentialsProvider;
import software.amazon.awssdk.regions.Region;
import software.amazon.awssdk.services.s3.S3Client;
import software.amazon.awssdk.services.s3.S3Configuration;
import software.amazon.awssdk.services.s3.model.CreateBucketRequest;
import software.amazon.awssdk.services.s3.model.HeadBucketRequest;
import software.amazon.awssdk.services.s3.model.S3Exception;
import software.amazon.awssdk.services.s3.presigner.S3Presigner;

@Configuration
public class S3Config {
    @Bean
    public S3Client s3Client(NotesProperties properties) {
        var s3 = properties.s3();
        return S3Client.builder()
            .endpointOverride(URI.create(s3.endpoint()))
            .region(Region.of(s3.region()))
            .credentialsProvider(StaticCredentialsProvider.create(
                AwsBasicCredentials.create(s3.accessKey(), s3.secretKey())))
            .serviceConfiguration(S3Configuration.builder()
                .pathStyleAccessEnabled(s3.pathStyleAccess())
                .build())
            .build();
    }

    @Bean
    public S3Presigner s3Presigner(NotesProperties properties) {
        var s3 = properties.s3();
        return S3Presigner.builder()
            .endpointOverride(URI.create(s3.endpoint()))
            .region(Region.of(s3.region()))
            .credentialsProvider(StaticCredentialsProvider.create(
                AwsBasicCredentials.create(s3.accessKey(), s3.secretKey())))
            .serviceConfiguration(S3Configuration.builder()
                .pathStyleAccessEnabled(s3.pathStyleAccess())
                .build())
            .build();
    }

    @Bean
    public ApplicationRunner ensureBucket(S3Client s3Client, NotesProperties properties) {
        return args -> {
            String bucket = properties.s3().bucket();
            try {
                s3Client.headBucket(HeadBucketRequest.builder().bucket(bucket).build());
            } catch (S3Exception ex) {
                s3Client.createBucket(CreateBucketRequest.builder().bucket(bucket).build());
            }
        };
    }
}
