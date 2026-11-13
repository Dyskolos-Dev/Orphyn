// @vitest-environment node
import { describe, expect, it } from 'vitest'
import type { CastMedia } from '@shared/cast'
import { parseAirplayFeatures, airplayDeviceFromService } from './airplay-txt'
import { castCapabilities, castDeviceFromService, parseCastTxt } from './cast-txt'
import { buildDidl, formatUpnpTime, parseUpnpTime } from './didl'
import { parseDeviceDescription } from './description'
import { buildSoapEnvelope, parseSoapResponse, SoapFaultError, soapActionHeader } from './soap'
import { buildCastMedia, mapCastStatus } from './transports/chromecast'
import { parsePlaybackInfo } from './transports/airplay'
import { escapeXml, findNode, parseXml, unescapeXml } from './xml'

const media: CastMedia = {
  url: 'http://jelly.local:8096/Audio/1/stream?static=true&api_key=abc',
  mimeType: 'audio/flac',
  kind: 'audio',
  title: 'Tom & Jerry <live> "2024"',
  artist: "Guns N' Roses",
  album: 'A > B',
  artworkUrl: 'http://jelly.local:8096/Items/1/Images/Primary?tag=x&maxWidth=300',
  durationSec: 3725,
}

describe('xml helpers', () => {
  it('escapes and unescapes the five entities and numeric references', () => {
    const raw = `a & b < c > d " e ' f`
    expect(escapeXml(raw)).toBe('a &amp; b &lt; c &gt; d &quot; e &apos; f')
    expect(unescapeXml(escapeXml(raw))).toBe(raw)
    expect(unescapeXml('&#65;&#x42;&amp;lt;')).toBe('AB&lt;')
  })

  it('parses nested elements, attributes, CDATA, namespaces and self-closing tags', () => {
    const root = parseXml(
      `<?xml version="1.0"?><!-- c --><a:root xmlns:a="x" id='1'><a:t>x &amp; y</a:t><e/><d><![CDATA[<raw>]]></d></a:root>`,
    )
    const r = findNode(root, 'root')!
    expect(r.attrs.id).toBe('1')
    expect(findNode(r, 't')?.text).toBe('x & y')
    expect(findNode(r, 'e')?.children).toHaveLength(0)
    expect(findNode(r, 'd')?.text).toBe('<raw>')
  })
})

describe('DIDL-Lite', () => {
  it('builds metadata with escaped fields, art, protocolInfo and duration', () => {
    const didl = buildDidl(media)
    expect(didl).toContain('<dc:title>Tom &amp; Jerry &lt;live&gt; &quot;2024&quot;</dc:title>')
    expect(didl).toContain('<upnp:artist>Guns N&apos; Roses</upnp:artist>')
    expect(didl).toContain('<upnp:album>A &gt; B</upnp:album>')
    expect(didl).toContain(
      'albumArtURI>http://jelly.local:8096/Items/1/Images/Primary?tag=x&amp;maxWidth=300<',
    )
    expect(didl).toContain('object.item.audioItem.musicTrack')
    expect(didl).toContain('protocolInfo="http-get:*:audio/flac:')
    expect(didl).toContain('duration="1:02:05"')
    expect(didl).toContain('stream?static=true&amp;api_key=abc</res>')
  })

  it('is well-formed XML that round-trips the title', () => {
    const item = findNode(parseXml(buildDidl(media)), 'title')
    expect(item?.text).toBe(media.title)
  })

  it('uses the movie class for video and adds a DLNA profile for mp3', () => {
    const d = buildDidl({ ...media, kind: 'video', mimeType: 'audio/mpeg' })
    expect(d).toContain('object.item.videoItem.movie')
    expect(d).toContain('DLNA.ORG_PN=MP3')
  })

  it('formats and parses UPnP times', () => {
    expect(formatUpnpTime(65.4)).toBe('0:01:05')
    expect(parseUpnpTime('1:02:05')).toBe(3725)
    expect(parseUpnpTime('0:00:01.500')).toBe(1.5)
    expect(parseUpnpTime('NOT_IMPLEMENTED')).toBe(0)
    expect(parseUpnpTime(undefined)).toBe(0)
  })
})

describe('SOAP', () => {
  it('builds an envelope with escaped arguments (DIDL embedded as text)', () => {
    const xml = buildSoapEnvelope(
      'urn:schemas-upnp-org:service:AVTransport:1',
      'SetAVTransportURI',
      [
        ['InstanceID', 0],
        ['CurrentURI', 'http://h/a?x=1&y=2'],
        ['CurrentURIMetaData', buildDidl(media)],
      ],
    )
    expect(xml).toContain(
      '<u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">',
    )
    expect(xml).toContain('<InstanceID>0</InstanceID>')
    expect(xml).toContain('<CurrentURI>http://h/a?x=1&amp;y=2</CurrentURI>')
    expect(xml).not.toContain('<DIDL-Lite')
    expect(xml).toContain('&lt;DIDL-Lite')
    // Round trip: the embedded metadata decodes back to the DIDL document.
    const meta = findNode(parseXml(xml), 'CurrentURIMetaData')
    expect(meta?.text).toBe(buildDidl(media))
    expect(soapActionHeader('urn:s', 'Play')).toBe('"urn:s#Play"')
  })

  it('parses responses and faults', () => {
    const ok = parseSoapResponse(
      `<s:Envelope xmlns:s="x"><s:Body><u:GetPositionInfoResponse xmlns:u="y"><RelTime>0:00:12</RelTime><TrackDuration>0:03:00</TrackDuration></u:GetPositionInfoResponse></s:Body></s:Envelope>`,
    )
    expect(ok).toEqual({ RelTime: '0:00:12', TrackDuration: '0:03:00' })
    const fault = `<s:Envelope xmlns:s="x"><s:Body><s:Fault><faultstring>UPnPError</faultstring><detail><UPnPError><errorCode>701</errorCode><errorDescription>Transition not available</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>`
    expect(() => parseSoapResponse(fault)).toThrow(SoapFaultError)
    expect(() => parseSoapResponse(fault)).toThrow(/701.*Transition not available/)
  })
})

describe('UPnP device description', () => {
  const xml = `<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0"><URLBase>http://192.168.1.50:49152/</URLBase>
<device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
<friendlyName>Salon &amp; Cuisine</friendlyName><manufacturer>Linkplay Technology Inc.</manufacturer>
<modelName>WiiM Mini</modelName><UDN>uuid:abc-123</UDN>
<serviceList>
<service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/upnp/control/rendertransport1</controlURL></service>
<service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>upnp/control/rendercontrol1</controlURL></service>
<service><serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType><controlURL>/cm</controlURL></service>
</serviceList></device></root>`

  it('extracts identity, control URLs and flags WiiM', () => {
    const d = parseDeviceDescription(xml, 'http://192.168.1.50:49152/description.xml')!
    expect(d.friendlyName).toBe('Salon & Cuisine')
    expect(d.udn).toBe('uuid:abc-123')
    expect(d.isWiim).toBe(true)
    expect(d.avTransportControlUrl).toBe('http://192.168.1.50:49152/upnp/control/rendertransport1')
    expect(d.renderingControlUrl).toBe('http://192.168.1.50:49152/upnp/control/rendercontrol1')
  })

  it('resolves relative URLs against the location when URLBase is missing, and rejects junk', () => {
    const noBase = xml
      .replace(/<URLBase>.*<\/URLBase>/, '')
      .replace('Linkplay Technology Inc.', 'Sonos')
      .replace('WiiM Mini', 'One')
    const d = parseDeviceDescription(noBase, 'http://10.0.0.2:1400/xml/device_description.xml')!
    expect(d.isWiim).toBe(false)
    expect(d.avTransportControlUrl).toBe('http://10.0.0.2:1400/upnp/control/rendertransport1')
    expect(parseDeviceDescription('<html></html>', 'http://x/')).toBeNull()
  })
})

describe('Cast discovery TXT', () => {
  it('reads friendly name, model and capability flags from TXT (strings or buffers)', () => {
    const svc = {
      name: 'Chromecast-abc',
      host: 'abc.local',
      port: 8009,
      addresses: ['fe80::1', '192.168.1.20'],
      txt: { id: 'abc', fn: Buffer.from('Living Room TV'), md: 'Chromecast', ca: '201221' },
    }
    const dev = castDeviceFromService(svc)!
    expect(dev).toMatchObject({
      id: 'chromecast:abc',
      name: 'Living Room TV',
      kind: 'chromecast',
      model: 'Chromecast',
      address: '192.168.1.20',
      port: 8009,
    })
    expect(dev.capabilities.video).toBe(true) // 201221 & 1
  })

  it('treats ca without the video bit as audio-only, and falls back to the model', () => {
    expect(castCapabilities(parseCastTxt({ ca: '198660' }))).toEqual({ audio: true, video: false })
    expect(castCapabilities(parseCastTxt({ md: 'Google Home Mini' }))).toEqual({
      audio: true,
      video: false,
    })
    expect(castCapabilities(parseCastTxt({ md: 'Chromecast Ultra' }))).toEqual({
      audio: true,
      video: true,
    })
  })

  it('needs an address', () => {
    expect(castDeviceFromService({ name: 'x', port: 1, txt: {} })).toBeNull()
  })
})

describe('Cast media builder and status mapping', () => {
  it('uses MusicTrackMediaMetadata for audio and MovieMediaMetadata for video', () => {
    const a = buildCastMedia(media) as { metadata: Record<string, unknown>; contentType: string }
    expect(a.metadata).toMatchObject({
      metadataType: 3,
      title: media.title,
      artist: media.artist,
      albumName: 'A > B',
    })
    expect(a.metadata.images).toEqual([{ url: media.artworkUrl }])
    expect(a.contentType).toBe('audio/flac')
    const v = buildCastMedia({ ...media, kind: 'video' }) as { metadata: Record<string, unknown> }
    expect(v.metadata.metadataType).toBe(1)
    expect(v.metadata.artist).toBeUndefined()
  })

  it('maps player states, finish and error', () => {
    expect(
      mapCastStatus({ playerState: 'PLAYING', currentTime: 4, media: { duration: 9 } }),
    ).toEqual({
      status: 'playing',
      positionSec: 4,
      durationSec: 9,
    })
    expect(mapCastStatus({ playerState: 'IDLE', idleReason: 'FINISHED' }).ended).toBe(true)
    expect(mapCastStatus({ playerState: 'IDLE', idleReason: 'ERROR' }).error).toBeTruthy()
    expect(mapCastStatus({ playerState: 'IDLE', idleReason: 'CANCELLED' }).status).toBe('idle')
  })
})

describe('AirPlay parsing', () => {
  it('parses the features bitfield and flags playable devices', () => {
    expect(parseAirplayFeatures('0x5A7FFFF7,0x1E') & 1n).toBe(1n)
    expect(parseAirplayFeatures(undefined)).toBe(0n)
    const tv = airplayDeviceFromService(
      {
        name: 'Apple TV',
        port: 7000,
        addresses: ['10.0.0.9'],
        txt: { deviceid: 'AA:BB:CC:00:11:22', features: '0x5A7FFFF7,0x1E', model: 'AppleTV6,2' },
      },
      'airplay',
    )!
    expect(tv.id).toBe('airplay:AABBCC001122')
    expect(tv.capabilities).toEqual({ audio: true, video: true })
    const raop = airplayDeviceFromService(
      { name: 'AABBCC001122@Apple TV', port: 49152, addresses: ['10.0.0.9'], txt: {} },
      'raop',
    )!
    expect(raop.id).toBe(tv.id)
    expect(raop.name).toBe('Apple TV')
    expect(raop.capabilities).toEqual({ audio: false, video: false })
  })

  it('does not mark Mac or HomePod receivers playable (AirPlay 2 only)', () => {
    for (const model of ['MacBookPro17,1', 'AudioAccessory5,1']) {
      const d = airplayDeviceFromService(
        {
          name: 'x',
          port: 7000,
          addresses: ['10.0.0.1'],
          txt: { deviceid: 'AA:BB:CC:00:11:33', features: '0x5A7FFFF7,0x1E', model },
        },
        'airplay',
      )!
      expect(d.capabilities).toEqual({ audio: false, video: false })
    }
  })

  it('parses playback-info plists', () => {
    const plist = `<plist><dict><key>duration</key><real>201.5</real><key>position</key><real>12.25</real><key>rate</key><real>1.0</real><key>readyToPlay</key><true/></dict></plist>`
    expect(parsePlaybackInfo(plist)).toEqual({
      rate: 1,
      position: 12.25,
      duration: 201.5,
      readyToPlay: true,
    })
    expect(parsePlaybackInfo('<plist><dict/></plist>')).toEqual({
      rate: 0,
      position: 0,
      duration: 0,
      readyToPlay: false,
    })
  })
})
