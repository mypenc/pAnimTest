//! Minimal MJPEG AVI muxer: one video stream of JPEG frames at a constant frame rate.

fn u32le(v:&mut Vec<u8>,x:u32){
    v.extend_from_slice(&x.to_le_bytes());
}

fn u16le(v:&mut Vec<u8>,x:u16){
    v.extend_from_slice(&x.to_le_bytes());
}

fn chunk(id:&[u8;4],body:&[u8])->Result<Vec<u8>,String>{
    let mut c=Vec::with_capacity(body.len()+9);
    c.extend_from_slice(id);
    u32le(&mut c,u32::try_from(body.len()).map_err(|_|"video too large for AVI")?);
    c.extend_from_slice(body);
    if body.len()%2==1 {
        c.push(0);
    }
    Ok(c)
}

fn list(kind:&[u8;4],body:&[u8])->Result<Vec<u8>,String>{
    let mut b=kind.to_vec();
    b.extend_from_slice(body);
    chunk(b"LIST",&b)
}

pub(crate) fn mux(w:u32,h:u32,fps:f32,jpegs:&[Vec<u8>])->Result<Vec<u8>,String>{
    let n=u32::try_from(jpegs.len()).map_err(|_|"too many frames")?;
    let fps=if fps.is_finite()&&fps>0.0 {fps} else {30.0};
    let (rate,scale)=((fps*1000.0).round().max(1.0) as u32,1000u32);
    let usf=(1e6/fps).round().max(1.0) as u32;
    let maxf=jpegs.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let mut avih=Vec::new();
    for x in [usf,0,0,0x10,n,0,1,maxf,w,h,0,0,0,0]{ //avih fields
        u32le(&mut avih,x);
    }
    let mut strh=Vec::new();
    strh.extend_from_slice(b"vidsMJPG");
    u32le(&mut strh,0);
    u16le(&mut strh,0);
    u16le(&mut strh,0);
    for x in [0,scale,rate,0,n,maxf,u32::MAX,0]{
        u32le(&mut strh,x);
    }
    for x in [0u16,0,w.min(65535) as u16,h.min(65535) as u16]{
        u16le(&mut strh,x);
    }
    let mut strf=Vec::new();
    for x in [40u32,w,h]{
        u32le(&mut strf,x);
    }
    u16le(&mut strf,1);
    u16le(&mut strf,24);
    strf.extend_from_slice(b"MJPG");
    for x in [w.saturating_mul(h).saturating_mul(3),0,0,0,0]{
        u32le(&mut strf,x);
    }
    let mut strl=chunk(b"strh",&strh)?;
    strl.extend(chunk(b"strf",&strf)?);
    let mut hdrl=chunk(b"avih",&avih)?;
    hdrl.extend(list(b"strl",&strl)?);
    let mut movi=Vec::new();
    let mut idx=Vec::new();
    for j in jpegs{
        let off=u32::try_from(movi.len()+4).map_err(|_|"video too large for AVI")?;
        movi.extend(chunk(b"00dc",j)?);
        idx.extend_from_slice(b"00dc");
        for x in [0x10u32,off,j.len() as u32]{
            u32le(&mut idx,x);
        }
    }
    let mut body=b"AVI ".to_vec();
    body.extend(list(b"hdrl",&hdrl)?);
    body.extend(list(b"movi",&movi)?);
    body.extend(chunk(b"idx1",&idx)?);
    let mut out=b"RIFF".to_vec();
    u32le(&mut out,u32::try_from(body.len()).map_err(|_|"video too large for AVI")?);
    out.extend(body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_size(){
        let a=mux(4,4,24.0,&[vec![1,2,3],vec![4,5]]).unwrap();
        assert_eq!(&a[..4],b"RIFF");
        assert_eq!(&a[8..12],b"AVI ");
        assert_eq!(u32::from_le_bytes(a[4..8].try_into().unwrap()) as usize,a.len()-8);
    }
}
