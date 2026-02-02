# ADR-0005: External File Storage for File Attachments

**Date**: 2026-02-04

**Status**: accepted

**ID**: `fdd-chat-engine-adr-file-handling`

## Context and Problem Statement

Users need to attach files to messages (images, documents, code files) for context-aware AI responses. Where should file content be stored, and how should Chat Engine handle file data as messages flow through the system?

## Decision Drivers

* File sizes can be large (up to 10MB per file, 50MB per message)
* Chat Engine focuses on message routing and tree management, not file storage
* Storage costs should be optimized (file storage cheaper than database)
* Webhook backends need direct file access for processing
* Clients should upload files quickly without Chat Engine bottleneck
* Infrastructure complexity should be minimized
* File durability and availability requirements match file storage capabilities

## Considered Options

* **Option 1: Separate File Storage service** - Clients upload to File Storage service, messages contain file URLs
* **Option 2: Database BLOB storage** - File content stored in PostgreSQL as bytea/BLOB columns
* **Option 3: Chat Engine file service** - Chat Engine provides upload endpoint, stores files on disk/storage

## Decision Outcome

Chosen option: "Separate File Storage service", because it eliminates file handling from Chat Engine critical path, leverages optimized file storage infrastructure, enables direct client uploads reducing latency, allows webhook backends direct file access, and minimizes Chat Engine storage and bandwidth costs.

### Consequences

* Good, because clients upload to File Storage service (presigned URLs) bypassing Chat Engine
* Good, because Chat Engine only stores small file URLs (not large file content)
* Good, because File Storage service provides file management with durability, availability, and CDN integration
* Good, because webhook backends can download files directly from File Storage
* Good, because File Storage service manages storage optimization
* Good, because Chat Engine infrastructure remains simple (no file storage management)
* Bad, because requires external file storage service deployment and configuration
* Bad, because file URLs must be signed with expiration (security complexity)
* Bad, because file lifecycle management is separate from session lifecycle
* Bad, because clients must implement upload-then-message-send flow

## Related Design Elements

**Actors**:
* `fdd-chat-engine-actor-file-storage` - Separate File Storage service managing file uploads and downloads
* `fdd-chat-engine-actor-client` - Uploads files to storage, includes URLs in messages
* `fdd-chat-engine-actor-webhook-backend` - Downloads files from storage using URLs

**Requirements**:
* `fdd-chat-engine-fr-attach-files` - Messages support file_urls array field
* `fdd-chat-engine-nfr-file-size` - Limits enforced by storage service, not Chat Engine
* `fdd-chat-engine-nfr-response-time` - File handling off critical path

**Design Elements**:
* `fdd-chat-engine-entity-message` - Contains file_urls (string array) not file content
* `fdd-chat-engine-constraint-external-storage` - Design constraint mandating separate File Storage service
* `fdd-chat-engine-design-context-file-storage` - Implementation details for presigned URLs

**Related ADRs**:
* ADR-0006 (Webhook Protocol) - File URLs forwarded to backends in message payload
* ADR-0007 (Database Architecture) - Database not used for file content storage
